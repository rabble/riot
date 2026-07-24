<!-- source-sha256: 047263bf850e11dca5e50ba8288f43a8a32d9c99b02b68ff829ea7930d179675 -->
# data safety.md

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
- Expected: all referenced network evidence rows exist
- Recovery: Correct the store answer or add the missing network evidence.

## privacy.google

- State: **PASS**
- Observed: first-launch,denied-permission,granted-permission,nearby-sync,followed-site-refresh
- Expected: all referenced network evidence rows exist
- Recovery: Correct the store answer or add the missing network evidence.

## privacy.required-reason-audit

- State: **HUMAN ACTION**
- Observed: 0
- Expected: completed submitted-dependency API inventory
- Recovery: Complete the Apple required-reason API audit before archive production.
