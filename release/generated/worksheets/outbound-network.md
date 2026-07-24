<!-- source-sha256: c0241a07706e9e8a80f07a8010827b0d6ccdbd048faa5eb371c7986e725f7101 -->
# outbound network

## first-launch

- Initiator: application
- Destination: none
- Transmitted fields: none
- Redirect handling: No request or redirect is made on first launch.
- Retention: No remote retention.
- Developer operated: false
- Developer retention: none
- User directed: false
- Code evidence: apps/ios/Riot/RiotApp.swift, apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt

## denied-permission

- Initiator: user
- Destination: none
- Transmitted fields: none
- Redirect handling: Denied permission does not initiate a request.
- Retention: Permission state remains platform-local.
- Developer operated: false
- Developer retention: none
- User directed: true
- Code evidence: apps/ios/Riot/QRScannerView.swift

## granted-permission

- Initiator: user
- Destination: local-device-subsystem
- Transmitted fields: permission-scoped sensor or discovery data
- Redirect handling: Platform permission APIs do not follow web redirects.
- Retention: Riot does not operate a remote retention service.
- Developer operated: false
- Developer retention: none
- User directed: true
- Code evidence: apps/ios/Riot/QRScannerView.swift, apps/ios/Riot/Transport/LocalNetworkNearby.swift

## nearby-sync

- Initiator: user
- Destination: user-selected nearby peer
- Transmitted fields: signed community records, peer discovery metadata
- Redirect handling: Direct local-network transport does not follow HTTP redirects.
- Retention: The receiving peer retains records under its local community state.
- Developer operated: false
- Developer retention: peer-local
- User directed: true
- Code evidence: apps/ios/Riot/Transport/LocalNetworkNearby.swift, crates/riot-transport/src/lib.rs

## followed-site-refresh

- Initiator: user
- Destination: followed public site
- Transmitted fields: HTTP request metadata
- Redirect handling: Transport policy validates the followed destination; redirect behavior remains subject to candidate audit.
- Retention: Destination-server retention is outside Riot developer control.
- Developer operated: false
- Developer retention: destination-controlled
- User directed: true
- Code evidence: crates/riot-core/src/site/follow.rs, apps/ios/Riot/FollowSiteModel.swift

# Gate summary

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
