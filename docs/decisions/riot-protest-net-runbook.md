# `riot.protest.net` public gateway prerequisites

This repository currently contains a local-only, stateless reader for the
versioned `incident-board/1` conference export. It is not deployed and this
document does not authorize or describe a deployment.

Before any deployment is considered, all of these prerequisites must exist:

- An approved, repository-documented hosting path with an accountable service
  owner and a supported Python runtime for `apps/gateway/server.py`.
- A DNS and TLS owner for `riot.protest.net`; DNS records, certificates, API
  tokens, and credentials must remain outside the repository.
- A release-approved public export whose revision and SHA-256 are recorded
  from the same conference fixture/sync boundary. The export must pass the
  gateway public-boundary tests without modification.
- Edge/origin policy that exposes only the read-only `/site/` routes and
  rejects write methods. No private-group routes, canonical content store,
  signer/key material, or administrative endpoint may be attached.
- An egress policy and deployment review confirming rendering needs no remote
  fetch, remote code, or external content source.
- A designated operator to run the local smoke check and independently record
  the deployed revision/content hash after an approved deployment path exists.

Local foundation check:

```sh
scripts/conference/gateway-smoke.sh
```
