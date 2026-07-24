<!-- source-sha256: de883918bcbba873346e75a98427ebdfe93359b1bad3826bf8a309a7878351d2 -->
# required reason apis

## Required-reason API inventory

No approved API reasons are recorded. Audit the submitted Apple archive and all dependencies before production.

# Gate summary

## privacy.apple

- State: **PASS**
- Observed: first-launch,denied-permission,granted-permission,nearby-sync,followed-site-refresh
- Expected: store answer agrees with position, transmitted fields, destination ownership, retention, and user direction
- Recovery: Correct the store answer or its canonical network evidence.

## privacy.google

- State: **PASS**
- Observed: first-launch,denied-permission,granted-permission,nearby-sync,followed-site-refresh
- Expected: store answer agrees with position, transmitted fields, destination ownership, retention, and user direction
- Recovery: Correct the store answer or its canonical network evidence.

## permission.camera

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical permission entry
- Recovery: Add exactly one camera permission justification and remove substitutions or duplicates.

## permission.bluetooth

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical permission entry
- Recovery: Add exactly one bluetooth permission justification and remove substitutions or duplicates.

## permission.local-network

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical permission entry
- Recovery: Add exactly one local-network permission justification and remove substitutions or duplicates.

## permission.notifications

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical permission entry
- Recovery: Add exactly one notifications permission justification and remove substitutions or duplicates.

## permission.android-internet

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical permission entry
- Recovery: Add exactly one android-internet permission justification and remove substitutions or duplicates.

## privacy.required-reason-audit

- State: **HUMAN ACTION**
- Observed: 0
- Expected: completed submitted-dependency API inventory
- Recovery: Complete the Apple required-reason API audit before archive production.
