<!-- source-sha256: 4b812f3e19089e0a916936334936f84394659546c35ae2fbe128a940554cef3a -->
# content rating

## Recommended ratings

- Apple: 12+
- Google Play: Teen
- State: human-action
- Rationale: Recommend these ratings because Riot contains user-generated content and community reporting; authenticated store questionnaires still require human confirmation.

## Claims considered

- Open a local community and read its newswire. (ios, macos, android; human-action)
- Create and switch communities. (ios, macos, android; human-action)
- Compose, sign, and persist an update. (ios, macos, android; human-action)

# Gate summary

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

- State: **PASS**
- Observed: Public support page (marketing/support/index.html) names the responsible operator and public contact channel.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence publicContact in a separately approved product workstream.

## policy.reportAcknowledgement

- State: **PASS**
- Observed: Public support page commits to acknowledging every report on the public email channel within 24 hours.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence reportAcknowledgement in a separately approved product workstream.

## policy.imminentHarm

- State: **PASS**
- Observed: Public support page commits to a 24-hour decision for imminent-harm or illegal-content reports on the public channel.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence imminentHarm in a separately approved product workstream.

## policy.objectionableContent

- State: **PASS**
- Observed: Public support page commits to a 72-hour decision for other objectionable-content reports on the public channel.
- Expected: implemented control with evidence, owner, and required SLA
- Recovery: Implement and evidence objectionableContent in a separately approved product workstream.

## claim.read-local-community

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: platform-specific code and candidate journey evidence for every claimed platform
- Recovery: Run candidate journeys ios:first-install-to-first-read, macos:first-install-to-first-read, android:first-install-to-first-read.

## claim.create-community

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: platform-specific code and candidate journey evidence for every claimed platform
- Recovery: Run candidate journeys ios:create-switch-community, macos:create-switch-community, android:create-switch-community.

## claim.publish-signed-update

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: platform-specific code and candidate journey evidence for every claimed platform
- Recovery: Run candidate journeys ios:publish-signed-update, macos:publish-signed-update, android:publish-signed-update.

## content-rating

- State: **HUMAN ACTION**
- Observed: 12+; Teen
- Expected: authenticated store questionnaires confirm the canonical recommendation
- Recovery: Confirm the content-rating questionnaires in App Store Connect and Google Play Console.
