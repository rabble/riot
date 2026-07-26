# Prosocial Local Spaces: Front Porch Forum, the Local Connection Crisis, and Roundabout

Date: 2026-07-24
Method: Source synthesis — three primary artifacts (the New_Public / Center for Media Engagement Front Porch Forum study, the Nov 2025 "Local Connection Crisis" survey, and New_Public's Roundabout launch material) plus the 14 Civic Signals framework. This is a literature/synthesis pass, not a deep-research workflow; claims carry source citations rather than adversarial vote counts.

## Purpose

This continues the community-design thread opened in `docs/research/2026-07-10-mutual-aid-coordination-research.md` and `docs/research/2026-07-11-disaster-riot-mutual-aid-evidence-research.md`. Those passes grounded riot's *crisis-response* design in evidence (disasters, protests, mutual aid). This pass looks at the adjacent question riot is increasingly adjacent to: **what makes a healthy *everyday* local digital space**, and what does the prosocial-local-spaces literature imply for a shutdown-resistant, offline-first, activism-focused tool?

Riot is not Roundabout and is not trying to be — riot's center of gravity is activism, internet shutdowns, and degraded connectivity, not neighborhood recommendations. But the problems converge: both are reactions to the failure of engagement-extractive social platforms to serve real human groups, and both conclude that **bounded, human-moderated, non-extractive spaces** beat algorithmic feeds. The transferable lessons are below.

## Summary

Three independently produced bodies of work converge on the same structural answer for what makes a local digital space healthy:

1. **Bounded, finite content** — not an infinite feed. Front Porch Forum (FPF) ships issues "once a week to three times a day" depending on volume; 63% of members read *every* issue because there *is* an "every issue." People abandon mainstream local spaces over "endless scrolling through an enshittified maze."
2. **Known, named human stewards** — moderation by people you can identify, not an algorithm or an anonymous admin flag. The Local Connection Crisis measured a real *named-by-memory* effect: belonging scores rise when users can recall their steward's name, above and beyond merely having a moderator present.
3. **Non-extractive incentives** — no engagement-for-profit loop. FPF is a public-benefit corporation that explicitly states it is "not in the surveillance business." Roundabout is a nonprofit. The top reasons people *leave* local spaces (toxicity, spam, notification overload, rage) are symptoms of the engagement-extraction model that prosocial spaces structurally avoid.

The single most counterintuitive and load-bearing finding from the FPF data: **forums that were more active and more positive were rated *less* favorably**, while longer-tenure members and donors rated it *more* favorably. Maximum engagement ≠ maximum value. The platform that works is the one people *don't* live inside. This is empirical support — from a 13,473-respondent survey — for riot's offline-first, sync-bounded model.

For riot specifically: the engagement-addiction risk that New_Public's framework explicitly warns against (the "Nourish" and "Take a Breath" Civic Signals) is **structurally lower** for riot by design. A peer-to-peer, sync-triggered model has no server pushing a feed and no algorithm optimizing for time-on-device. Riot should treat that as a design asset to preserve, not a gap to close.

## The three sources

### Front Porch Forum study (New_Public / CME, 2024)

FPF is a Vermont public-benefit corporation running town-specific, **proactively moderated** online forums — moderators review all content *before publication*. 235,000+ active members across Vermont (plus parts of NY/MA). The 2023 survey had 13,473 partial / 11,465 complete responses.

FPF's own stated values, quoted in the report:
- Not in the surveillance business; no profiling or data selling.
- Vermont-based moderators review all content before publication.
- Does not fuel screen addiction; meant as a starting point for real-world engagement.

**Civic Signals performance.** FPF members rated FPF against the 14 Civic Signals and against Facebook and Nextdoor on the same signals. FPF outperformed both on every signal. Representative gaps (5-point scale):

| Signal | FPF | Facebook | Nextdoor |
|---|---|---|---|
| Connected to local area | 4.36 | 2.79 | 3.25 |
| People treat each other humanely | 4.07 | 2.41 | 3.03 |
| Likely to become a more informed citizen | 4.06 | 2.60 | 2.98 |
| Feel safe | 4.05 | 2.46 | 3.04 |
| Reliable information | 3.99 | 2.33 | 3.09 |
| Information is secure | 3.58 | 2.03 | 2.88 |

**Real-world impact.** Members reported taking civic actions because of FPF: 60.6% attended a local event/public meeting, 53.2% discussed local issues with a neighbor, 51.1% bought from a local business, 25.3% let a neighbor borrow something, 21.0% volunteered locally, 19.2% cooperated on a shared community need.

**The counterintuitive engagement finding.** Controlling for member demographics and forum attributes:
- Members of forums that were *more active* rated FPF *less* favorably (3.55) than members of less-active forums (3.84).
- Members of forums with *higher positivity* rated FPF *less* favorably (3.62) than lower-positivity forums (3.73) — though higher-positivity members had a stronger *personal* sense of community.
- Longer-tenure members and donors rated FPF *more* favorably on every outcome.
- Newcomers to a neighborhood found FPF *more* useful than long-time residents.

Read together: FPF's value comes from being a trusted, completable, recurring resource, not from maximizing throughput. Activity and positivity, past a point, erode the experience.

### The Local Connection Crisis (New_Public / CME, Nov 2025)

A survey of 2,028 people (506 suburban, 502 urban, 1,020 mixed/rural) on how they use and perceive local digital spaces today. Three headline takeaways:

1. **People value local digital spaces but they could be better.** Facebook is the most-used local-connectivity platform. Most people's most-used local space is small (<1,000 people) and city/town-scoped.
2. **There's an interest gap.** Across all three geographies, >⅔ of people are interested in using a digital platform for local recommendations and events, but <½ are currently doing so. The largest rural gap is coordinating around weather emergencies.
3. **People leave over toxicity and friction.** The top five self-reported leave-reasons: (1) lack of interest/relevance, (2) negativity/harassment/scams, (3) spam/notifications/annoying content, (4) values disagreement/misalignment, (5) personal life changes. Three of five are symptoms of the engagement-extraction model.

**The steward effect.** Just over half of local digital spaces have a steward (admin/moderator). Where one exists — and especially where users can *name the steward by memory* — belonging scores and overall experience ratings rise, controlling for other factors. 28–33% of respondents would serve as a paid steward for ~8 hrs/week; only 1–18% would pay even $10/month.

### Roundabout (New_Public, 2025)

New_Public's own response product, now in closed beta across five pilot communities (Burlington NC, Richmond VA, Lincoln County WI, North Chattanooga TN, Conestoga Valley PA). Targeted at 10,000–50,000-person towns/neighborhoods. Built on the AT Protocol.

Design philosophy:
- **"Doorway, not destination"** — screens bridge to offline, real-world connection; the point is to "actually meet, build ties and take action together," not to maximize screen time.
- **Stewardship and co-design** — every community is started and built by a local steward; the first cohort got 3 months of training, 3x weekly, on community listening, content strategy, moderation, and growing membership.
- **Nonprofit, anti-enshittification** — structured to avoid the "cycle of enshittification" and the "rage cycle"; incentives aligned to "utility and everyday value."

Feature set:
- **Channels** — topic-based, not algorithmic (Event Calendar, Local Guides, Help & Recs, Info & Updates, Biz Openings, Education, Parenting, New to Town).
- **Local Guides** — Wikipedia-style evergreen community knowledge base, so the same questions don't get re-asked.
- **Participatory event calendar** — any resident can photograph a flyer and upload an event.
- **Highlights homepage** — curated landing page, no algorithmic feed.
- **Community agreement** required on join; software auto-flags violations with guaranteed human review.

## The 14 Civic Signals framework

New_Public and CME's research-validated framework for healthy digital public spaces, organized in four building blocks. This is the most directly reusable artifact — a ready-made scorecard for prosocial design. Riot should evaluate against it explicitly.

| Block | Signal | Riot-relevant reading |
|---|---|---|
| **Welcome** | Open to All | Join-by-reference + QR already built; no-account model lowers the barrier |
| | Safety | Moderation model; panic wipe; compartmentalization |
| | Identity | Pseudonymous collectives as publishers; unlinkable private groups |
| | Nourish | **Structurally strong for riot** — no feed, no engagement optimization |
| **Connect** | Shared Experience | Community/site as the shared object |
| | A Place of Our Own | Owned namespaces, per-community subspace |
| | Mutual Recognition | Steward as a named, visible role (see implication #2) |
| **Understand** | Common Knowledge | Trusted reference content in packets; correction/rumor-control objects |
| | Context | Provenance, signer display, expiry on operational objects |
| | Take a Breath | **Structurally strong** — sync-bounded, not real-time-push |
| | Take In the Whole Picture | Open newswire as a shared baseline |
| **Act** | Empowered | Authoring + signing from inside the installed app |
| | Useful | Operational objects (alerts, needs, resources) with real-world use |
| | Self-Governing | `/mod/` records, owner-signed moderation, Meadowcap delegation |

The two signals riot satisfies *by transport design* rather than by policy are **Nourish** and **Take a Breath** — exactly the two that engagement-extractive platforms structurally cannot satisfy. That is riot's distinctive contribution relative to Roundabout (which satisfies them by nonprofit discipline) and to FPF (which satisfies them by moderation labor).

## Design implications for Riot

These are framed for riot's actual mission — activism, shutdowns, degraded connectivity, mutual aid — not for becoming a neighborhood-recommendations app.

1. **Treat "no feed" as a feature, and design the unit of consumption as bounded and completable.** FPF's self-throttling (issues, not a stream) is empirically tied to its high Civic Signals scores. Riot's sync model naturally produces bounded batches if the UI treats a sync as "here's what's new since you last looked" with a finite end, rather than an infinite timeline. "Finished reading" should be a reachable, satisfying state. *This is the "doorway, not destination" principle, and riot already implements it by accident — make it deliberate.* The "what's-new / unread" work in `engagement-gap-map.md` (a per-device last-seen cursor diffed against the projection) is exactly the right primitive; frame it as a *finite delta*, not a feed position.

2. **Make stewardship a first-class, named, visible role in the data model — not an admin boolean.** The Local Crisis *named-by-memory* effect is the evidence: belonging rises when users can recall their steward's name. For riot this maps to: a steward's identity (human-facing name, presence) should propagate as part of a community's public state, not be hidden behind a role flag. In riot's terms, a site/community owner or designated moderator should be *visible as a person* on the read surface. This reinforces the existing `/mod/` owner-signed moderation design — the human behind the moderation should be seen, not just the moderation actions.

3. **The interest gap is a discovery problem, and offline-first solves it differently — but it must be solved.** >⅔ of people want local recommendations/events; <½ get them. This is the single biggest unmet need the literature identifies. Riot's peer graph is naturally scoped to a community, which means discovery is *who you're connected to*, not *what an algorithm surfaces*. That's a strength for trust but a weakness for serendipity. Implication: if riot's sites/packets carry FPF-style categories (events, recs, help, free/sell, civic) and evergreen "local guides," those should be **synced artifacts** — the community's accumulated knowledge travels with the peer set. The "local guides" concept is especially resonant with riot's packet/site model: a curated, evergreen reference site that syncs is exactly what Willow was designed for.

4. **Don't optimize for activity; optimize for trusted-and-completable.** FPF's data shows high-activity forums score *worse* on satisfaction. A riot community that is "quiet but trusted" is a success state, not a failure state. **Resist any metric that reads like DAU/MAU.** The right success metrics for riot are closer to: "did this operational update reach the people who needed it," "is the moderation fresh," "can members name their steward," "did a real-world action result." This is a direct argument *against* importing conventional social-product engagement metrics into riot's roadmap.

5. **Adopt the Civic Signals as an explicit design scorecard.** They're research-validated and free to adopt. Riot already satisfies several by transport design (Nourish, Take a Breath). The ones to actively design for are **Mutual Recognition** (steward visibility), **Self-Governing** (the `/mod/` surfacing work already scoped), and **Useful** (operational objects with real-world use — already the packet examples). Score riot against all 14 in the next design review.

6. **The "leave reasons" are riot's anti-persona spec.** Toxicity, spam, notification overload, and rage are the top abandonment drivers. Three of four are *symptoms of engagement extraction* that riot structurally avoids. This is a testable design constraint, not a marketing claim: *can a riot peer spam a community?* The answer should be "no, by transport + moderation design" — and that should be verifiable, not asserted.

## What riot should *not* take from Roundabout

Riot is not building a neighborhood-recommendations product, and the Roundabout feature surface (biz openings, parenting channels, curated email newsletters) is mostly out of scope. The transferable parts are the *structural* lessons (bounded content, named stewards, non-extractive incentives, Civic Signals), not the *content vertical*. Riot's analog of a "community" is an affinity group, a mutual-aid network, or a protest/disperse cell — the stewardship model transfers, the channel taxonomy does not.

Equally: Roundabout's "open architecture on the AT Protocol" is, today, aspirational — one nonprofit operator, hand-curated stewards. Riot's offline-first transport makes federation a *physical fact*: the network is bounded by who you're actually connected to, and there is no central server to seize or optimize. That is riot's distinctive position relative to Roundabout and is worth being deliberate about in positioning.

## The steward-economy question (open)

The Local Crisis data shows a real tension: 28–33% of people would *serve* as a steward for pay (~8 hrs/week), but only 1–18% would *pay* even $10/month. Roundabout is experimenting with local sponsorships. Riot's decentralized, no-server model makes centralized billing structurally hard — but it also makes peer-to-peer steward compensation possible (and interesting). For riot's activism use cases, stewardship is more likely to map to a designated organizer/medic/legal-lead role than a paid neighborhood moderator. Worth deciding early whether riot's stewards are volunteer, movement-funded, or P2P-compensated, because it shapes the role model and the trust dynamics. Flagged as open, not resolved here.

## Open questions

- How does the "named steward → higher belonging" effect translate when the steward is a pseudonymous collective (riot's publication-space model) rather than a named individual? Is there an analogous "recognizable, accountable presence" affordance for collective publishers?
- Does FPF's "high activity correlates with lower satisfaction" finding hold under riot's sync-bounded model, or does the relationship invert when there's no engagement-extraction baseline to compare against? Worth instrumenting if riot ever runs a field deployment.
- What is riot's equivalent of "local guides" (evergreen, synced community knowledge) in the packet/site model, and does it already exist as a packet type, or is it net-new?

## Primary sources

- New_Public / Center for Media Engagement, "Front Porch Forum: Fostering Civic Engagement and Building Community in Vermont" (2024) — local PDF attachment; also https://newpublic.org/study/3635/front-porch-forum
- New_Public / Center for Media Engagement, "The Local Connection Crisis: New Data on What Communities Need" (Nov 2025) — local PDF attachment
- New_Public, "Introducing Roundabout: built for neighbors, with neighbors" — https://newpublic.substack.com/p/introducing-roundabout-built-for
- "Building Roundabout: A Pro-Social Platform for Local Communities" (prosocialdesign.org) — https://www.prosocialdesign.org/blog/building-roundabout-a-pro-social-platform-for-local-communities
- Roundabout product site — https://joinroundabout.com/
- New_Public, "Our Civic Signals research" — https://newpublic.org/signals
- Center for Media Engagement, "Civic Signals: The Qualities of Flourishing Digital Spaces" — https://mediaengagement.org/research/civic-signals-the-qualities-of-flourishing-digital-spaces/

## Cross-references within riot docs

- `docs/research/2026-07-10-mutual-aid-coordination-research.md` — the intake → verify → dispatch pipeline and TXTMob channel matrix this builds on.
- `docs/research/2026-07-11-disaster-riot-mutual-aid-evidence-research.md` — cites FPF as a methodological reference; the consumer-tool-incumbent and trust-is-social findings there are reinforced here.
- `docs/engagement-gap-map.md` — the "what's-new / unread" Swift-only work is the primitive for the bounded-delta consumption model described in implication #1.
- `docs/product/product-brief.md` — riot's "What Riot Is Not" (not a general social network, not a chat app, no server) aligns with the non-extractive design lesson; worth cross-linking.
