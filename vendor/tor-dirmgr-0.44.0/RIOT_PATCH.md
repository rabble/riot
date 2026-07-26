# Riot patch for tor-dirmgr 0.44.0

This directory is the source of the `tor-dirmgr` 0.44.0 crate published by
the Tor Project on crates.io.

Riot changes one line in `Cargo.toml`: the `rusqlite` upper bound is widened
from `<0.40.0` to `<0.41.0`. Arti uses APIs that remain available in
`rusqlite` 0.40.1, while Riot requires that release to preserve its pinned
bundled SQLite 3.53.2 engine.

Do not make source changes here. Replace this directory from a newer upstream
release and remove the workspace patch as soon as Arti supports rusqlite 0.40
or later directly.
