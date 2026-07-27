# deploy/riot-anchor/Dockerfile — the riot-anchor daemon.
#
# Build from the REPOSITORY ROOT (the workspace Cargo.lock is the build input):
#
#   docker build -f deploy/riot-anchor/Dockerfile -t riot-anchor:dev .
#
# The builder image matches the workspace toolchain pin (rust-toolchain.toml:
# channel = "1.95.0", edition 2021) so rustup does not download a second
# toolchain inside the build. `--locked` builds exactly the committed
# Cargo.lock — the same lock the repo's validate-contracts gate pins.
# `deploy/riot-anchor/Dockerfile.dockerignore` keeps target/, .git/, and the
# app trees out of the build context.

FROM rust:1.95-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p riot-anchor --features daemon --locked

FROM debian:bookworm-slim
# ca-certificates: iroh dials the n0 relay/discovery infrastructure over TLS.
# SQLite is bundled into the binary (rusqlite bundled build) — no libsqlite3.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --home /var/lib/riot-anchor anchor \
    && mkdir -p /var/lib/riot-anchor \
    && chown anchor /var/lib/riot-anchor
COPY --from=build /src/target/release/riot-anchor /usr/local/bin/riot-anchor
USER anchor
# The database path is the --db CLI FLAG (not an environment variable); the
# secrets and descriptor metadata come from the environment — see
# riot-anchor.example.env and crates/riot-anchor/src/config.rs (the authority).
#
# Exec-form ENTRYPOINT makes riot-anchor PID 1, so it receives signals directly
# (no shell wrapper to swallow them). Docker's default STOPSIGNAL is already
# SIGTERM, which the daemon now catches (see main.rs `shutdown_signal`) to run
# the graceful lease-relinquish + clean-close path — so no STOPSIGNAL override
# is needed. `docker compose stop`/`restart` therefore exit cleanly (0/143)
# instead of being SIGKILLed (137).
ENTRYPOINT ["riot-anchor"]
CMD ["--db", "/var/lib/riot-anchor/anchor.sqlite3"]
