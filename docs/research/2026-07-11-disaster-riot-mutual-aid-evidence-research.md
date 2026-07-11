# What People Actually Need: Disaster, Protest, and Mutual-Aid Evidence Pass 2

Date: 2026-07-11
Method: Deep-research workflow — 5 search angles, 23 sources fetched, 96 claims extracted, top 25 adversarially verified (3 independent verification votes per claim), merged to 10 synthesized findings. 18 of 25 votes confirmed, 7 refuted. Findings below are only the survivors; refuted claims are listed separately so they are not mistaken for established.

## Purpose

This continues `docs/research/2026-07-10-mutual-aid-coordination-research.md`, which established the core intake → verify → dispatch pipeline, the shared-editable-ledger pattern, the TXTMob channel matrix, Indymedia's hide-not-delete moderation, and the NYC COVID mutual-aid consumer-tool stack — treated here as settled priors, not re-researched.

The brief for this pass, in the spirit of New_Public's [Front Porch Forum study](https://newpublic.org/study/3635/front-porch-forum) — grounding design in close, evidence-based accounts of what real people needed from a real platform in a real crisis or organizing context, not abstract workflow diagrams — was to (1) close the four open coverage holes the prior doc named, and (2) find additional case studies with direct participant testimony.

## Summary

Non-crisis mutual aid (timebanks, community fridges) is bottlenecked less by missing software features than by psychological and behavioral friction: discomfort asking for help, and an attitude-behavior gap where people who intend to request help act on it, but people who intend to offer help often don't. Movement media operating under an active state ban achieve censorship resistance chiefly through infrastructure placement — mirroring blocked content onto commercial CDNs a censor can't afford to fully block — not through novel peer-to-peer protocols, which leaves an open gap for an offline-first tool like Riot with no internet to hide inside. Real infrastructure collapse at scale (the 2023 Turkey-Syria earthquakes, 2017 Hurricane Maria) shows a consistent pattern: consumer chat apps remain the default even for trained professional responders, satellite phones are a scarce resource controlled by outside teams rather than locals, ham radio operates as a pre-existing institutional emergency-relay backbone activated through formal partner requests rather than ad hoc hobbyist improvisation, and fallback-channel preference varies by community rather than following one universal ladder. Private-group infiltration defense in practice is almost entirely social and embodied — in-person vouching and face-to-face behavioral judgment — not technical, and carries a documented paradox where visibly adopting a maximally secure tool can itself mark someone as a suspected infiltrator. A fifth case study, Telegram in the 2019 Hong Kong protests, independently reinforces the prior pass's TXTMob finding: many parallel, functionally differentiated channels (broadcast tactical-intel channels vs. deliberative discussion channels) outperform a single general-purpose channel, and no single channel held sustained coordinating authority.

Coverage-hole status: (b) censorship-resistant publishing and (c) infrastructure-down fallbacks are now well-evidenced with primary sources. (d) private-group vetting/infiltration defense is well-evidenced, but only for one movement tradition (UK climate activism). (a) day-to-day non-crisis mutual aid is partially closed — timebanks and community fridges are covered; worker and housing cooperatives were not found in this pass and remain open.

## Verified Findings

### Non-crisis mutual aid (timebanks, community fridges)

- **Timebank participation is limited by psychological friction, not missing features.** Heavy emphasis on tracking time debits/credits fails to dispel members' discomfort with asking for help. There is also an asymmetry: a positive attitude toward *requesting* services predicts actually requesting them, but a positive attitude toward *offering* services does not predict actually offering them. [merged 2-1 and 3-0] (Han, Shih, Bellotti & Carroll, CI 2014, year-long fieldwork across hOurworld/TimeBanks USA/Community Forge; Information Technology & People 2019, 120+-timebank TAM survey)
- **Community fridge networks coordinate day-to-day pickups and cleaning shifts through an ordinary consumer tool** (a public Slack workspace, e.g. LA Community Fridges), not bespoke mutual-aid software, and volunteer burnout results when people move directly from paid day jobs into scheduling/cleaning shifts with no structural break. [2-1] (The Counter, corroborated by LA Taco)
- *Refuted, do not rely on:* that existing timebank software specifically lacks support for spontaneous, near-real-time exchanges [1-2]; that fridge organizers explicitly instruct volunteers to disable Slack notifications as a burnout countermeasure [1-2] — the burnout pattern is real, this particular mitigation is not confirmed as standard practice.

### Movement media under an active state ban

- **Reporters Without Borders' primary censorship-circumvention method is infrastructure placement, not protocol novelty**: mirroring a banned outlet's content onto major commercial CDN servers, so that blocking the mirror means blocking the whole cloud provider. The "Collateral Freedom" program currently mirrors 177 outlets across 39 countries; 57% of the blocking activity (102 of 177 sites) is concentrated in just two countries, China (53) and Russia (49). [3-0, merged] (RSF primary disclosures, corroborated by IFEX/ecoi.net republication)
- This pattern does not transfer directly to an offline-first/mesh model: there is no CDN to hide inside once the internet itself is down or blocked at the ISP level. Riot's censorship-resistance story for an active-ban scenario needs a different mechanism than the RSF pattern.

### Infrastructure collapse at scale (Turkey-Syria 2023, Puerto Rico/Maria 2017)

- **Even trained professional responders fell back to WhatsApp.** In the immediate aftermath of the Feb 2023 Turkey-Syria earthquakes, loss of connectivity and destroyed/lost phones disrupted normal channels; international, domestic, and nonprofit search-and-rescue teams relied primarily on WhatsApp for local coordination. Satellite phones were scarce and reserved exclusively for outside teams that brought their own devices — local teams' own satellite equipment was itself disrupted by the earthquake. [3-0, merged] (Insecurity Insight/CDAC Network interview study; Natural Hazards Center Quick Response Report, published in *Earthquake Spectra*)
- **Radio fallback use was demographically uneven, not universal.** It was specifically cited as gaining significance by Syrian refugee youth and men in the same earthquake response, and was not mentioned at all by Turkish respondents in the same study. [3-0] (Insecurity Insight, 44 participants across 8 group interviews + 13 key-informant interviews)
- **Ham radio functioned as an activated institutional backbone, not ad hoc improvisation.** After Hurricane Maria, the American Red Cross formally requested ARRL's assistance; ARRL deployed a coordinated "Force of 50" volunteer operators island-wide, and ARRL's own station W1AW suspended its regular scheduled broadcasts specifically to relay outbound health-and-welfare traffic. Independent reporting describes this as an unprecedented request scale in the ~75-year Red Cross-ARRL relationship. [merged 2-1 and 3-0] (ARRL primary archive, corroborated by CNN, NBC News)
- *Refuted, do not rely on:* that ~250 portable base stations were deployed and took about a day to restore Turkish cell service [0-3]; that residents coordinated informally via WhatsApp and rescue teams used geolocated Twitter to dispatch helicopters to stranded people [1-2].

### Private-group vetting and infiltration defense (UK climate movement)

- **Infiltration defense in practice is social and embodied, not technical.** UK climate activists said deceit is harder to disguise face-to-face than online, and formed affinity groups in person specifically to vet against police infiltrators using personal cues. The named mechanisms activist communities actually use are "vouching" (a member formally attesting a prospective member is committed to the group's purpose, trustworthy, reliable, and accountable) and strict compartmentalization ("if people don't know anything, they can't talk about it" — only active participants in an action should know about it, and never over phone/email/mail). [merged 3-0, 2-1, 2-1] (2025 ethnographic study, 15 interviews + participant observation; Direct Action Movement; Sprout Distro security-culture handbook)
- **Adopting a highly secure tool can itself be a suspicion signal.** UK climate groups run a two-tier trust model: public channels assume every member is potentially malicious and are kept free of sensitive information, while small private affinity groups assume all members are honest by default. This creates a documented paradox — because participants believe undercover police ("spy cops") use the same secure tools, visibly using Signal can itself mark someone as a possible infiltrator. [merged 2-1, 3-0] (same 2025 ethnographic study, grounded in the UK's documented "spy cops" undercover-policing history)
- *Refuted, do not rely on:* a defined minimum-relationship-history standard for a valid vouch [0-3]; explicit rejection of weak-tie vouching [1-2]; a named typology of infiltrator behavior patterns in security-culture handbooks [1-2].

### Case study: Telegram in the 2019 Hong Kong Anti-Extradition Bill movement

- **A single platform supported both broadcast and deliberation modes.** Telegram combined admin-controlled broadcast "channels" with open-forum "groups," plus encryption/secret-chat/unsend/anonymous-forwarding features. The movement's channel network functionally split into "Authority" channels (real-time tactical intelligence like police movements, >50% of content) versus "Hub" channels (deliberation/strategy, ~40% of content). [merged 2-1, 3-0] (Urman, Ho & Katz, PLOS ONE 2021, HITS-based network analysis)
- **The movement was structurally leaderless**: no single channel or organization maintained sustained authority/hub ranking — different channels topped the rankings each month across an 8-month analysis — and at least one dominant channel explicitly disclaimed a coordinating role ("This channel is not a central stage, we will not suggest or instruct anyone to take any action"). [2-1] (same source, also released as preprint "No Central Stage")
- **Caveat found independently**: Telegram group chats were not end-to-end encrypted by default in 2019, and a phone-number-matching flaw let authorities identify at least one coordinator — the "layered anonymity" story describes the feature set as designed/marketed, not a guarantee that held up against a motivated state adversary.

## Cross-Cutting Patterns

1. **Consumer tools are the incumbent, not a blank slate.** WhatsApp, Slack, and Telegram are what people actually reach for — even trained SAR professionals and community organizers — before any purpose-built tool. Riot competes against generic chat already in use, not against "no tool."
2. **Trust and vetting are social acts the app can support but not perform.** Membership admission in practice is in-person vouching and behavioral judgment; no observed community substitutes a technical verification step for that judgment.
3. **Visible security posture is itself a social signal, and more is not always safer.** A group whose members are suspected of infiltration risk can read forced maximal encryption as a marker of who's hiding something, inverting the intended trust effect.
4. **Scarce hardware (satellite phones, portable base stations) is externally supplied and locally uncontrolled**, while radio-relay capacity is pre-existing volunteer infrastructure activated through formal institutional partnership, not spontaneous hobbyist mutual aid.
5. **Fallback-channel preference is community-specific, not a universal ladder** — the same disaster produced different fallback habits for Syrian refugees versus Turkish citizens in the same interview study.
6. **Many parallel, functionally differentiated channels beat one general channel**, confirmed independently in a second dataset (Telegram/Hong Kong platform-usage-log analysis) beyond the prior pass's TXTMob interview evidence.
7. **Today's censorship-resistance playbook (CDN mirroring) is an internet-present strategy** and does not obviously transfer to an internet-absent or ISP-blocked scenario — a real, unresolved gap for Riot.
8. **Request-side and offer-side friction in mutual aid are not symmetric** — people act more readily on intent to ask than on intent to give.

## Design Implications for Riot

- **Don't build symmetric request/offer UX.** Lowering the barrier to *asking* (the `request`/`need` object) is higher leverage than adding features to encourage offering; visible debit/credit-style ledgers may increase self-consciousness about asking rather than reduce it.
- **Differentiate from chat rather than reproduce it.** Riot's mutual-aid surface should offer what generic group chat structurally can't — shift/capacity limits on `task` claims, expiry-driven quiet periods, load-shedding — since Slack/WhatsApp-class tools are already the incumbent and win on familiarity alone.
- **Model group admission as a social act the app records, not one it verifies.** A `vouched_by` field and an invite-provenance/audit trail (who invited whom, when) fit the observed practice; the app's job is to carry compartmentalized, need-to-know information flow once trust is established out-of-band (matches the existing Meadowcap path-scoped capability design), not to establish that trust itself.
- **Let groups calibrate visible security posture instead of forcing one default.** Given the Signal-as-suspicion-signal paradox, a private group's UI should not mandate the most visible/maximal security affordance uniformly — expose posture as a per-group choice tied to the group's own threat model.
- **Treat satellite/backup hardware and ham-radio relay as externally supplied, institutionally activated resources**, not something Riot's design can assume end users possess or can informally recruit ad hoc. Any "off-grid relay" story for Riot should describe a formal partner-activation flow (mirroring the Red Cross → ARRL request pattern) rather than assuming any nearby operator can be pulled in spontaneously.
- **Make the offline/degraded fallback channel configurable per community rather than fixed.** Given the Turkish/Syrian radio-use split, Riot should not hard-code one universal "when data drops, use X" ladder — it should let a community's pre-existing fallback habits (radio, paper, runners) be configured or learned.
- **Reinforce (independently) the existing multi-channel design decision.** The Telegram/Hong Kong finding is a second, independent dataset supporting the dual-mode/TXTMob-matrix architecture already adopted: build for many parallel, functionally distinct spaces (broadcast alert channels vs. deliberative discussion channels) rather than one general-purpose space, and avoid any design that concentrates coordination in a single "main" channel.
- **Treat "Collateral Freedom"-style censorship resistance as unsolved for Riot's actual scenario.** RSF's CDN-mirroring approach assumes a still-connected-but-blocked reader; Riot's offline-first/mesh model needs its own mechanism (sneakernet/mesh relay, bridging into CDN-mirrored channels only when partial connectivity exists, or another approach) rather than assuming the RSF pattern transfers — flagged as an open design question, not a solved one.

## Coverage Holes and Open Questions

- What sync/distribution design gives Riot RSF-style censorship resistance when there is no internet at all to hide a CDN mirror inside?
- How do worker co-ops and housing co-ops (as distinct from timebanks and community fridges) coordinate day-to-day, and does their governance/consensus tooling suggest anything different from the patterns found here? (Not found in this pass — still open.)
- Is there interview/practitioner-testimony evidence — as opposed to platform-usage-log research like the Telegram/Hong Kong study — explaining *why* organizers chose the channel-vs-group, Authority-vs-Hub split, i.e. what they say they needed, not just what the data shows emerged?
- Does the UK climate movement's "adopting Signal marks you as a suspected infiltrator" paradox generalize to US-based or lower-surveillance-history mutual-aid/disaster contexts, or is it specific to movements with a well-documented undercover-policing history like the UK's "spy cops" scandal?

### Sourcing caveats

The Turkey-earthquake and UK-climate-activism findings each rest on a single qualitative/ethnographic study (44 participants / 15 interviews respectively) — rich in direct testimony but not statistically representative, and both are single-national-context studies whose specific splits (Turkish-vs-Syrian radio use; UK "spy cops" suspicion dynamics) may not generalize elsewhere. The RSF Collateral Freedom statistics are a March-2026 operational snapshot that will drift over time. Several load-bearing findings above survived at 2-1 rather than unanimous 3-0 (marked accordingly) — treat those as directionally solid but slightly less airtight than the unanimous ones.

Case-study leads searched but not yielding claims that survived to the final synthesis: Zello/Cajun Navy during Hurricane Harvey and FireChat during the 2014 Hong Kong Umbrella Movement were both fetched with source claims extracted (Fox 7 Austin, CNN, Time) but did not survive the top-25 adversarial verification cut in this pass — worth a dedicated follow-up pass if Riot's mesh-transport design needs FireChat-specific evidence.

## Primary Sources

- Han, Shih, Bellotti & Carroll, "Timebanking with a Smartphone Application," Collective Intelligence 2014 — https://patshih.luddy.indiana.edu/publications/Han-MobileTimebanking-CI14.pdf
- "Assessing timebanking use and coordination," Information Technology & People, 2019 — https://www.emerald.com/itp/article-abstract/32/2/344/179198/Assessing-timebanking-use-and-coordination
- The Counter, "Community fridges aren't a pandemic fad — they're entrenched in neighborhoods facing hunger" — https://thecounter.org/community-fridges-not-pandemic-fad-entrenched-neighborhoods-hunger/
- RSF, "Collateral Freedom: 57% of censored news sites mirrored by RSF are blocked by Russia or China" — https://rsf.org/en/collateral-freedom-57-censored-news-sites-mirrored-rsf-are-blocked-russia-or-china
- RSF, "Collateral Freedom" program page — https://rsf.org/en/collateral-freedom
- Insecurity Insight / CDAC Network, "Türkiye Communication and Community Engagement Ecosystem" (2023) — https://insecurityinsight.org/wp-content/uploads/2023/09/Turkiye-CCE-ecosystem.pdf
- Natural Hazards Center Quick Response Report, "Communication and Coordination Networks in the 2023 Kahramanmaraş Earthquakes" — https://hazards.colorado.edu/quick-response-report/communication-and-coordination-networks-in-2023-kahramanmaras-earthquakes
- ARRL, "Hurricane Maria 2017" archive — https://www.arrl.org/hurricane-maria-2017
- CNN, "Puerto Rico ham radio operators" (2017) — https://www.cnn.com/2017/09/27/us/puerto-rico-maria-ham-radio-operators-trnd/index.html
- NBC News, "Puerto Rico amateur radio operators play key role" (2017) — https://www.nbcnews.com/news/latino/puerto-rico-amateur-radio-operators-are-playing-key-role-puerto-n805426
- "On the Virtues of Information Security in the UK Climate Movement" (arXiv, 2025) — https://arxiv.org/pdf/2506.09719
- Direct Action Movement, "Vouching" — https://www.thedirectactionmovement.com/vouching
- Sprout Distro, "Security Culture: A Handbook for Activists" — https://www.sproutdistro.com/catalog/zines/security/security-culture-a-handbook/
- Urman, Ho & Katz, "Telegram channels as a news media" / "No Central Stage," PLOS ONE 2021 — https://pmc.ncbi.nlm.nih.gov/articles/PMC8500451/
- Geographical, "The peril of big tech in disaster response" — https://geographical.co.uk/news/the-peril-of-big-tech-in-disaster-response
- Fox 7 Austin, "Zello app invaluable tool for Cajun Navy during Harvey response" — http://www.fox7austin.com/news/local-news/austin-based-zello-app-invaluable-tool-for-cajun-navy-during-harvey-response
- Time, "How FireChat helped Hong Kong's protesters organize" — https://time.com/3449812/hong-kong-protesters-firechat/
- CNN, "FireChat and the mesh network" (2014) — https://www.cnn.com/2014/10/16/tech/mobile/tomorrow-transformed-firechat/index.html
- New_Public, Front Porch Forum study (methodological reference for this research approach) — https://newpublic.org/study/3635/front-porch-forum
