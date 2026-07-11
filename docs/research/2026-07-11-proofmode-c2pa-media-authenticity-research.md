# Optional Media Authenticity: ProofMode, C2PA, and the Redaction-vs-Provenance Tension

Date: 2026-07-11
Method: Deep-research workflow — 6 search angles, 26 sources fetched, 114 claims extracted, top 25 adversarially verified (3 independent verification votes per claim), 18 confirmed / 7 refuted. **Note on this write-up:** the workflow's automated synthesis step failed twice in a row on this run, returning placeholder output instead of a real report (a recurring bug in this session — see the two censorship/distribution research docs from earlier today for the same failure mode). The underlying search → fetch → verify pipeline ran correctly both times; this document was hand-assembled directly from the intact verified-claim data (full claim text, supporting quotes, and vote tallies) recovered from the workflow's execution journal, with sources attributed by topical matching against the run's own fetched-source list rather than an exact per-claim source link (noted where attribution is inferred rather than machine-verified).

## Purpose

Riot's binary-media design is currently one sentence (`docs/superpowers/specs/2026-07-10-riot-dual-mode-design.md`). The project owner wants users who want it to be able to use **ProofMode** (Guardian Project's capture-time verifiable-metadata app) to produce **C2PA** (Content Credentials) authenticity data for media published through Riot — but this creates a concrete tension: C2PA/ProofMode provenance is designed to *preserve and disclose* capture metadata (GPS, device, timestamp) as proof of authenticity, while Riot's safety model needs the opposite for protest/dissident photography — strip exactly that metadata before publishing. This pass researched whether that tension has a real, spec-supported resolution, or whether it's a genuine unsolved conflict.

## Summary

It resolves, with a caveat. ProofMode's proof is already a detachable bundle — the original unmodified media file plus a separate SHA256 hash, a CSV sensor-data snapshot, and a PGP signature — not something baked into the media file itself, so it naturally fits Riot's existing `annotation`/`verification` object pattern rather than requiring new media-embedding machinery. C2PA independently and separately supports exactly the redaction Riot needs: assertions (including GPS) can be formally removed from a manifest when an asset is reused as an ingredient, without breaking the hash chain, and this redaction is not restricted to the original signer — a downstream party (Riot's own publish pipeline) can do it. C2PA's own implementation guidance goes further and recommends *against* simply stripping an embedded manifest, preferring it be "externalized" into a separate store instead of deleted — which is precisely Riot's existing pattern of detachable annotation objects. But there's a real, documented weakness sitting inside that same mechanism: the "exclusion range" that lets GPS be excluded from the cryptographic signature leaves those excluded bytes *unprotected* — anyone, not just the signer, can alter or insert false data there (a fake GPS location, for instance) without invalidating the signature or being flagged by a conforming validator. A validator saying "this manifest is valid" is therefore not sufficient grounds for trusting excluded/redacted fields. Separately, ProofMode's own identity model — a persistent, per-install OpenPGP keypair that signs every capture — is a real repeat-contributor correlation risk that directly contradicts the project's own stated design goal of not requiring persistent identity; ProofMode's documentation has no privacy/redaction mode or high-risk-use guidance of its own. ProofMode–C2PA integration is real but partial: ProofMode Verify can inspect and analyze C2PA-formatted metadata, but a stronger "full C2PA conformance" claim did not survive adversarial verification.

## Verified Findings

### ProofMode's technical shape and identity model

- **ProofMode's proof bundle is already detachable from the media file**: the original unmodified photo/video, a SHA256 hash, a CSV file of device/sensor data, and a PGP signature over that data — not a single opaque notarized blob baked into the asset. [3-0] This is good news for Riot's object model: it can be carried as a separate signed artifact, matching the existing `annotation`/`verification` pattern, rather than requiring the media payload itself to change shape.
- **ProofMode's signing identity is a persistent, per-app-install OpenPGP keypair that signs every capture** — the same key across all of a user's submissions, with no key-rotation practice documented. [3-0] This directly contradicts the project's own stated design goal of "not requiring a persistent identity or account" [3-0, cited against the same claim] — a real internal inconsistency, not just a theoretical risk: a repeat contributor's submissions are correlatable by that fixed key unless the user manually regenerates it.
- **ProofMode explicitly names activists among its intended users** for chain-of-custody/proof needs, per its own project description. [3-0]
- **ProofMode's documentation has no privacy, anonymization, redaction, or metadata-stripping mode, and no guidance for high-risk use cases** like protest or dissident photography — its design goals emphasize verification and chain-of-custody, not source protection. [3-0] Riot cannot rely on ProofMode itself to resolve the redaction tension; that responsibility has to live in Riot's own pipeline.
- ProofMode's own marketing language claims to give "control of all the metadata that you include, how it is shared, and what identity you use" [3-0], but this is a general framing claim, not a documented specific mechanism — treat it as a design intention, not a confirmed feature.
- ProofMode's product suite includes a separate "Identify" service for device verification and attestation signing [2-1] — a discrete infrastructure component with its own device-level cryptographic identity, relevant to (but distinct from) the capture-app correlation risk above.

### C2PA's redaction mechanism — real, spec-level, not signer-restricted

- **C2PA formally supports removing (redacting) specific assertions — including sensitive metadata — from an asset's manifest when the asset is reused as an ingredient**, without necessarily breaking the manifest's hash chain, provided the underlying content bytes are unchanged and an update manifest documents the redaction. [3-0, C2PA specification text] "Assertions that may contain sensitive information can be removed via redaction" is stated directly in the spec. [3-0]
- **This redaction is not restricted to the original signer** — any party handling the asset as it becomes an ingredient in a derived work can trigger removal of assertions, meaning a downstream publisher (Riot's own publish step, not just the original photographer) can in principle strip fields like GPS. [3-0]
- **C2PA's implementation guidance recommends against fully stripping an embedded manifest**, treating that as discouraged unless the manifest is being "externalized" — moved out of the file into a separate repository — rather than deleted outright. [3-0] This maps directly onto Riot's existing pattern of separate, linked annotation objects rather than payload-embedded metadata.
- **"Soft binding" is a manifest-*recovery* mechanism, not a redaction mechanism** — it exists because real-world distribution workflows often strip C2PA manifests from assets entirely, decoupling provenance data from the media file, and soft binding is how a manifest gets reconnected to its asset afterward. [3-0, both components] Don't conflate the two in Riot's own design language.

### The weakness inside the same mechanism

- **C2PA's "exclusion range" mechanism — which lets data like GPS be excluded from the cryptographic signature — leaves the excluded bytes unprotected**: anyone, not just the original signer, can alter or insert data in that range (including a false GPS location) without invalidating the signature or being detected by a conforming validator. [3-0, peer-reviewed source proposing an additional-signature fix] This is the central caveat for Riot: the exact mechanism that enables privacy-safe redaction is also an unauthenticated-tampering surface if a validator's "valid" result is treated as covering the redacted fields too.
- **C2PA manifests and invisible watermarking are independent, non-cross-checking verification layers**, enabling a documented "Integrity Clash": an asset can carry a valid C2PA manifest asserting human authorship while a separate watermark identifies it as AI-generated, with both checks individually passing. [3-0, peer-reviewed] A caution against treating any single authenticity signal — C2PA included — as sufficient on its own.
- **A documented attack requires no cryptographic compromise of C2PA at all** — it exploits the fact that the current spec permits semantic omission of a single assertion field, meaning selective non-disclosure is a spec-permitted mechanism that can be misused, not a implementation bug. [3-0]

### ProofMode–C2PA integration: real, but partial

- **ProofMode Verify supports inspecting and analyzing C2PA-formatted metadata**, alongside IPTC and EXIF — confirmed, direct compatibility at the read/inspection level. [3-0]
- *Refuted, do not rely on:* a claim that ProofMode (v3.x) claims full conformance with the C2PA 2.3 specification [1-2]. Treat ProofMode's C2PA support as "can read/inspect C2PA data," not "natively emits fully spec-conformant C2PA manifests" — that stronger claim did not survive adversarial review.
- *Refuted, do not rely on:* that ProofMode explicitly does not require persistent identity as an accurate description of current behavior [0-3], and that ProofMode's proof is a bare hash-plus-signature with no sensor-data component [0-3] — both contradicted by the fuller, confirmed bundle-shape finding above.

## Cross-Cutting Patterns

1. **Detachability is already the native shape on both sides.** ProofMode's own proof bundle is separate from the media file; C2PA's own guidance prefers externalized manifests over embedded-and-stripped ones. Riot doesn't need to invent a new mechanism to keep authenticity data optional and separable — it needs to *choose* the externalized/detached path both tools already lean toward.
2. **Privacy and authenticity keep turning out to be the same mechanism, not opposing ones — with a shared weak point.** C2PA's redaction and exclusion-range features are how privacy gets enforced, but they're also where an unauthenticated party can tamper without detection. Any Riot design that adds redaction has to also account for what "validated" no longer covers.
3. **Neither tool's own documentation addresses high-risk/source-protection use directly.** ProofMode's docs don't mention protest or dissident photography despite naming activists as a target user; the redaction mechanism exists in the C2PA spec for general reasons (ingredient reuse, sensitive-info removal), not because C2PA was designed with source protection as a primary goal.
4. **"This tool's own marketing/design-goal language" and "what the tool actually does" diverged more than once in this research** — ProofMode's stated goal of no persistent identity contradicted its own persistent per-install signing key; a claimed C2PA conformance claim didn't survive verification. Riot's design should verify claims about any third-party authenticity tool against primary technical documentation, not accept framing language at face value.

## Design Implications for Riot

- **Model ProofMode/C2PA authenticity as an optional, detachable `verification` annotation object**, not something embedded in or required by the media payload — this is directly supported by how both tools already shape their own data, and lets Riot's existing safety principle ("preview before ingest," provenance as a separate receipt) apply unchanged.
- **The default publish path should carry a redacted manifest, with full unredacted provenance retained privately.** A photographer captures with ProofMode/C2PA normally; the full manifest stays local or inside a private group for the photographer's own chain-of-custody needs; before anything crosses to the public newswire, Riot's existing Group→Newswire bridge review step (which already strips private identifiers from other object types before publish) does the same redaction C2PA's own spec supports — GPS and device-identifying assertions removed, human-reviewed, then published. This is a natural extension of a bridge pattern Riot already has, not a new one.
- **Do not let a "valid" C2PA check imply the redacted/excluded fields are trustworthy.** Riot's `verification` annotation for a media object should explicitly record *what was redacted and by whom*, not just a pass/fail validator result — the exclusion-range weakness means a redacted field is an active gap, not neutral missing data.
- **Surface ProofMode's persistent-key correlation risk to users, don't inherit it silently.** If Riot supports ProofMode capture, the UI should make clear that repeated use signs with the same identity across submissions unless the user takes explicit action — matching the general safety principle already in the product brief of not letting a "verified" workflow accidentally deanonymize the person who did the verifying.
- **Keep the terminology straight in Riot's own docs**: soft binding (manifest recovery) and redaction (privacy) are different C2PA mechanisms solving different problems — don't let them blur together in the object vocabulary or user-facing copy.
- **This is a genuinely optional layer, consistent with the "not an AI authority" / "trust as a lens" principles already in the product brief** — Riot should not require ProofMode/C2PA for publishing, and a `verification` annotation carrying this data should be one trust signal among the existing ones (source claims, eyewitness/N-source verification), not a new gate.

## Coverage Holes and Open Questions

- **No verified findings survived on prior art for reconciling provable authenticity with source protection in high-risk photojournalism/citizen journalism specifically** (research question 4). Sources were fetched — WITNESS's own C2PA harms-and-misuse assessment, Starling Lab, eyewitness.global, World Privacy Forum — but no claims from that angle reached the verified top-25 in this pass. This is a real gap, not a negative finding, and the WITNESS harms-and-misuse assessment in particular looks worth a dedicated follow-up pass given WITNESS's direct human-rights-documentation focus.
- **The exact signed-data shape for Riot's own `verification` object carrying this data was not directly resolved by a verified claim** (research question 5), though the ProofMode bundle-shape and C2PA externalization-preference findings above answer it indirectly — worth a short, scoped design pass rather than more research.
- Given the exclusion-range weakness, is there a documented, adopted mitigation (the "additional signatures" approach one source proposes) that's further along than a single academic proposal? Worth checking before Riot leans on C2PA redaction as a trusted mechanism rather than a caveated one.

### Sourcing caveat specific to this write-up

Because this document was reconstructed after the workflow's synthesis step failed, per-claim source URLs above are assigned by topical match against this run's fetched-source list (proofmode-android's GitHub README, `gitlab.com/guardianproject/proofmode`, `proofmode.org/metadata`, `proofmode.org/about`, `proofmode.org/c2pa`, the C2PA specification and guidance pages at `spec.c2pa.org`, and two arXiv papers on C2PA redaction/exclusion-range handling and on C2PA/watermark "Integrity Clash") rather than a machine-verified one-to-one claim-to-source link. The claim text and quotes themselves are recovered verbatim from the workflow's execution journal, not paraphrased or reconstructed from memory.

## Primary Sources

- ProofMode Android README — https://github.com/guardianproject/proofmode-android/blob/master/README.md
- ProofMode Android repository — https://github.com/guardianproject/proofmode-android
- ProofMode (GitLab) — https://gitlab.com/guardianproject/proofmode
- ProofMode app page (Guardian Project) — https://guardianproject.info/apps/org.witness.proofmode/
- ProofMode metadata page — https://proofmode.org/metadata
- ProofMode about page — https://proofmode.org/about
- ProofMode C2PA integration page — https://proofmode.org/c2pa
- ProofMode simple-c2pa (GitLab) — https://gitlab.com/guardianproject/proofmode/simple-c2pa
- ProofMode blog, "simple-c2pa" — https://proofmode.org/blog/simple-c2pa
- C2PA Specification 2.4 — https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html
- C2PA Security Considerations 1.0 — https://spec.c2pa.org/specifications/specifications/1.0/security/Security_Considerations.html
- C2PA Soft Binding / Decoupled Manifests 2.4 — https://spec.c2pa.org/specifications/specifications/2.4/softbinding/Decoupled.html
- C2PA Implementation Guidance 2.4 — https://spec.c2pa.org/specifications/specifications/2.4/guidance/Guidance.html
- arXiv, C2PA redaction/exclusion-range handling — https://arxiv.org/pdf/2603.02378
- arXiv, C2PA/watermark "Integrity Clash" and metadata-washing attack — https://arxiv.org/pdf/2604.24890
- CDT, "The Promise and Risk of Digital Content Provenance" — https://cdt.org/insights/the-promise-and-risk-of-digital-content-provenance/
- WITNESS, "WITNESS and the C2PA Harms and Misuse Assessment Process" — https://blog.witness.org/2021/12/witness-and-the-c2pa-harms-and-misuse-assessment-process/
- WITNESS, C2PA specifications assessment (PDF) — https://blog.witness.org/wp-content/uploads/2021/11/C2PA-Specifications-for-WITNESS-blog.pdf
- WITNESS, ProofMode background — https://blog.witness.org/2017/04/proofmode-helping-prove-human-rights-abuses-world/
- Starling Lab — https://starlinglab.org/about-us/
- World Privacy Forum, "Privacy, Identity, and Trust in C2PA" — https://worldprivacyforum.org/posts/privacy-identity-and-trust-in-c2pa/
- eyeWitness, "Using metadata" — https://www.eyewitness.global/Using-metadata
