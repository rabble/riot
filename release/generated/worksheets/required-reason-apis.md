<!-- source-sha256: 0226348d7fef6c716989998149b7accd731ed335a4b3385137a806355b929ada -->
# required reason apis

## Required-reason API inventory

No approved API reasons are recorded. Audit the submitted Apple archive and all dependencies before production.

# Gate summary

## inventory.privacy-answers

- State: **PASS**
- Observed: {"apple":1,"google":1}
- Expected: exactly one Apple and one Google privacy answer
- Recovery: Restore the exact canonical Apple and Google answer inventory.

## inventory.network-rows

- State: **PASS**
- Observed: {"first-launch":1,"denied-permission":1,"granted-permission":1,"nearby-sync":1,"followed-site-refresh":1}
- Expected: exactly one of: first-launch, denied-permission, granted-permission, nearby-sync, followed-site-refresh
- Recovery: Restore the exact canonical outbound-network scenario inventory.

## network.first-launch

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical evidence row
- Recovery: Add exactly one first-launch evidence row and remove duplicates or substitutions.

## network.denied-permission

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical evidence row
- Recovery: Add exactly one denied-permission evidence row and remove duplicates or substitutions.

## network.granted-permission

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical evidence row
- Recovery: Add exactly one granted-permission evidence row and remove duplicates or substitutions.

## network.nearby-sync

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical evidence row
- Recovery: Add exactly one nearby-sync evidence row and remove duplicates or substitutions.

## network.followed-site-refresh

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical evidence row
- Recovery: Add exactly one followed-site-refresh evidence row and remove duplicates or substitutions.

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
