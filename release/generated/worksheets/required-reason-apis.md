<!-- source-sha256: 68bbdfcd6ee983d4c355f82189a31798bb7cf1f30916a01e3f92f47b28d918e4 -->
# required reason apis

## Required-reason API inventory

No approved API reasons are recorded. Audit the submitted Apple archive and all dependencies before production.

# Gate summary

## inventory.privacy-answers

- State: **PASS**
- Observed: {"apple":1,"google":1}
- Expected: exactly one Apple and one Google privacy answer
- Recovery: Restore the exact canonical Apple and Google answer inventory.

## network.first-launch

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add the first-launch evidence row.

## network.denied-permission

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add the denied-permission evidence row.

## network.granted-permission

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add the granted-permission evidence row.

## network.nearby-sync

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add the nearby-sync evidence row.

## network.followed-site-refresh

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add the followed-site-refresh evidence row.

## privacy.apple

- State: **PASS**
- Observed: first-launch,denied-permission,granted-permission,nearby-sync,followed-site-refresh
- Expected: all exact scenarios with store-platform code evidence agree with position, fields, destination ownership, retention, and user direction
- Recovery: Correct the store answer or its canonical platform-specific network evidence.

## privacy.google

- State: **PASS**
- Observed: first-launch,denied-permission,granted-permission,nearby-sync,followed-site-refresh
- Expected: all exact scenarios with store-platform code evidence agree with position, fields, destination ownership, retention, and user direction
- Recovery: Correct the store answer or its canonical platform-specific network evidence.

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
