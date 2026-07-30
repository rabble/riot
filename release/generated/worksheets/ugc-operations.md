<!-- source-sha256: a00ad8d7ec3c99b2b4c6520576759a7edfa2c31eaf3121aef1d343565659d791 -->
# ugc operations

## termsAcceptance

- State: blocked
- Reason: No in-app Terms or user-policy acceptance flow was found.
- Operator: not assigned
- Maximum response hours: not applicable
- Code evidence: not implemented

## prohibitedContent

- State: blocked
- Reason: No published prohibited-content rules tied to the app were found.
- Operator: not assigned
- Maximum response hours: not applicable
- Code evidence: not implemented

## filtering

- State: blocked
- Reason: No objectionable-content filtering control was found.
- Operator: not assigned
- Maximum response hours: not applicable
- Code evidence: not implemented

## contentReporting

- State: blocked
- Reason: No in-app content-reporting flow was found.
- Operator: not assigned
- Maximum response hours: not applicable
- Code evidence: not implemented

## authorReporting

- State: blocked
- Reason: No in-app author-reporting flow was found.
- Operator: not assigned
- Maximum response hours: not applicable
- Code evidence: not implemented

## localBlocking

- State: blocked
- Reason: No immediate local author-blocking control was found.
- Operator: not assigned
- Maximum response hours: not applicable
- Code evidence: not implemented

## moderatorTombstone

- State: blocked
- Reason: Protocol tombstones exist, but no report-to-moderator response workflow was found.
- Operator: not assigned
- Maximum response hours: not applicable
- Code evidence: not implemented

## publicContact

- State: pass
- Reason: Public support page (marketing/support/index.html) names the responsible operator and public contact channel.
- Operator: rabble@protest.net
- Maximum response hours: not applicable
- Code evidence: marketing/support/index.html

## reportAcknowledgement

- State: pass
- Reason: Public support page commits to acknowledging every report on the public email channel within 24 hours.
- Operator: rabble@protest.net
- Maximum response hours: 24
- Code evidence: marketing/support/index.html

## imminentHarm

- State: pass
- Reason: Public support page commits to a 24-hour decision for imminent-harm or illegal-content reports on the public channel.
- Operator: rabble@protest.net
- Maximum response hours: 24
- Code evidence: marketing/support/index.html

## objectionableContent

- State: pass
- Reason: Public support page commits to a 72-hour decision for other objectionable-content reports on the public channel.
- Operator: rabble@protest.net
- Maximum response hours: 72
- Code evidence: marketing/support/index.html

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

## content-rating

- State: **HUMAN ACTION**
- Observed: 12+; Teen
- Expected: authenticated store questionnaires confirm the canonical recommendation
- Recovery: Confirm the content-rating questionnaires in App Store Connect and Google Play Console.
