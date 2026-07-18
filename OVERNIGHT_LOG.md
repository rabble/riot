# Overnight Log — 2026-07-19

_(Summary goes at the TOP when done. Task entries appended below in order.)_

## Setup / bearings
- Branch `overnight/2026-07-19` off `origin/main` (`ae9ec47`), isolated worktree (shared checkout — many concurrent sessions). Never commit to main, pathspec commits, no force-push.
- Docs read: `docs/coordination/2026-07-18-coordinator-status.md`, `docs/coordination/2026-07-18-ux-persona-workflow-audit.md`, CLAUDE.md/AGENTS.md conventions, COLLABORATION.md ledger.
- Skills: repo has no `skills/`; the plugin skills (superpowers brainstorming/writing-plans, metaswarm design/plan gates, TDD) are the SOP — used as applicable.
- **Lane chosen:** iOS UX completeness (my proven lane; the swarm is on gateway/web `/2` unification + composite-site Rust — I stay off those to avoid the cross-session duplication that bit the composite-site Unit 1 earlier). Owner-blocked items (TF hardware test, owner ratifications) skipped.
- **Note:** several UX-audit gaps were already closed today by the landed iOS-surface build (PR #42): join-by-link/QR, share-community, read-alerts, editorial-for-joined, add-a-tool, alert/request compose. Remaining iOS-UX gaps are the targets below.

## Candidate tasks (to ground then execute, riskiest-unknown first)
1. Editorial completeness — is Tombstone (6th action) intentionally unwired? wire it if not; + moderation/editorial-action audit view.
2. Display-name prominence — audit says the field EXISTS but may be buried; add an obvious entry point.
3. Onboarding / first-run flow — biggest gap; no named flow (install → identity → community → post).
4. "Open in Riot" verify landing (app side) — the differentiator; deep link exists, app-side "signature checks out" landing missing.

