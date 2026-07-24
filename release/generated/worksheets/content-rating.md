<!-- source-sha256: 976fa97d55e81f5854ae7c84508b2693c5c1c432c57ee5adead47fbc3a59bcb2 -->
# content rating.md

## policy.termsAcceptance

- State: **BLOCKED**
- Observed: No in-app Terms or user-policy acceptance flow was found.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence termsAcceptance in a separately approved product workstream.

## policy.prohibitedContent

- State: **BLOCKED**
- Observed: No published prohibited-content rules tied to the app were found.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence prohibitedContent in a separately approved product workstream.

## policy.filtering

- State: **BLOCKED**
- Observed: No objectionable-content filtering control was found.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence filtering in a separately approved product workstream.

## policy.contentReporting

- State: **BLOCKED**
- Observed: No in-app content-reporting flow was found.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence contentReporting in a separately approved product workstream.

## policy.authorReporting

- State: **BLOCKED**
- Observed: No in-app author-reporting flow was found.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence authorReporting in a separately approved product workstream.

## policy.localBlocking

- State: **BLOCKED**
- Observed: No immediate local author-blocking control was found.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence localBlocking in a separately approved product workstream.

## policy.moderatorTombstone

- State: **BLOCKED**
- Observed: Protocol tombstones exist, but no report-to-moderator response workflow was found.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence moderatorTombstone in a separately approved product workstream.

## policy.publicContact

- State: **BLOCKED**
- Observed: The required public support/report page and escalation path do not exist.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence publicContact in a separately approved product workstream.

## policy.reportAcknowledgement

- State: **BLOCKED**
- Observed: No owned report intake process can provide the required acknowledgement.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence reportAcknowledgement in a separately approved product workstream.

## policy.imminentHarm

- State: **BLOCKED**
- Observed: No named operator or escalation process can meet the 24-hour decision SLA.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence imminentHarm in a separately approved product workstream.

## policy.objectionableContent

- State: **BLOCKED**
- Observed: No named operator or response process can meet the 72-hour decision SLA.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence objectionableContent in a separately approved product workstream.

## claim.read-local-community

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: passing code and candidate journey evidence
- Recovery: Run candidate journeys first-install-to-first-read.

## claim.create-community

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: passing code and candidate journey evidence
- Recovery: Run candidate journeys create-switch-community.

## claim.publish-signed-update

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: passing code and candidate journey evidence
- Recovery: Run candidate journeys publish-signed-update.
