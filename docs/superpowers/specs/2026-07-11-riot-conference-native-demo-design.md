# Riot conference native demo design

## Fixed public package boundary

The conference fixture is a versioned, public incident-space package. It names
one communal Willow namespace, two full public author identifiers, a fixed
incident title, deterministic content metadata, and routes rendered only below
`/site/`. Identifiers are full-width hexadecimal values: 32-byte namespace,
public-key, and entry identifiers; signatures are full 64-byte values. The
fixture is encoded into a fixed canonical CBOR projection and pins its SHA-256
digest so changes are deliberate and reviewable.

`package-manifest-v1.json` is declarative data, never code. Its only renderer
profile is `incident-board/1`; it names the same public namespace and title and
allows only `alert`, `observation`, `resource`, `request`, and `offer` object
kinds. The package has no executable JavaScript, remote resource URLs, secrets,
or private identifiers. Native clients render this fixed profile locally rather
than loading arbitrary package code.

## Truthful native-demo scope

The demo uses Riot-owned, bounded nearby reconciliation; it makes no Willow
Transfer Protocol compatibility claim. It demonstrates a public space only and
makes no Confidential Sync or private-group security claim. Nearby transport,
preview, and acceptance are later demo layers; this fixture does not imply mesh
routing, server synchronization, or arbitrary remote-code execution.

Model assistance can produce an editable draft and the fixture visibly marks
that assistance. Model output remains draft-only: a person reviews it and signs
the resulting public content locally. A model cannot publish, import, sync, or
hold cross-space authority.
