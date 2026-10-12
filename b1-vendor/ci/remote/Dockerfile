# Gate image for the shared test host (ci/remote/job.sh; docs/BUILD.md "How each stage runs"). It holds only the
# toolchains and the cargo tools. The source is mounted at run time, and cargo's home, the target directory and the npm
# cache live in volumes. job.sh tags the image with the hash of this file and rust-toolchain.toml, so a toolchain change
# builds a new image.
FROM docker.io/library/debian:bookworm-slim

# The stable channel of rust-toolchain.toml (job.sh passes it).
ARG RUST_TOOLCHAIN
# The fuzz job runs `cargo +nightly`. A dated nightly is installed and symlinked as `nightly` (rustup refuses `nightly`
# as a custom toolchain name), so the image is reproducible.
ARG RUST_NIGHTLY=nightly-2026-09-30
ARG NODE_VERSION=22.21.1
ARG BUILD_JOBS=4

RUN apt-get update -qq && \
    apt-get install --no-install-recommends -y \
      build-essential ca-certificates curl git pkg-config procps xz-utils && \
    rm -rf /var/lib/apt/lists /var/cache/apt/archives

ENV RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:$PATH

RUN test -n "$RUST_TOOLCHAIN" && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
      CARGO_HOME=/usr/local/cargo sh -s -- -y -q --no-modify-path --profile minimal --default-toolchain none && \
    CARGO_HOME=/usr/local/cargo rustup toolchain install "$RUST_TOOLCHAIN" --profile minimal -c rustfmt -c clippy && \
    CARGO_HOME=/usr/local/cargo rustup toolchain install "$RUST_NIGHTLY" --profile minimal && \
    ln -s "$RUST_NIGHTLY-x86_64-unknown-linux-gnu" "$RUSTUP_HOME/toolchains/nightly-x86_64-unknown-linux-gnu" && \
    CARGO_HOME=/usr/local/cargo rustup default "$RUST_TOOLCHAIN"

# The versions of the Mac gate (docs/tooling.md). Tools live in /usr/local/cargo/bin; cargo finds them on PATH while
# CARGO_HOME points at the volume.
RUN CARGO_HOME=/usr/local/cargo CARGO_BUILD_JOBS=$BUILD_JOBS cargo install -q --locked \
      cargo-nextest@0.9.146 cargo-mutants@27.1.0 cargo-bolero@0.13.5 && \
    rm -rf /usr/local/cargo/registry /usr/local/cargo/git

RUN curl -fsSL "https://nodejs.org/dist/v$NODE_VERSION/node-v$NODE_VERSION-linux-x64.tar.xz" | \
      tar -xJ -C /usr/local --strip-components=1 --exclude CHANGELOG.md --exclude LICENSE --exclude README.md && \
    node --version && npm --version


# cargo-mutants builds its mutant copies under TMPDIR. The gate's /tmp is a tmpfs (job.sh: test temp roots off the disk,
# so writeback of a heavy writer in the same container cannot stall them), so this shim, first on PATH, gives
# cargo-mutants a directory on the target volume instead.
RUN mkdir -p /usr/local/botster/bin && \
    printf '%s\n' '#!/bin/sh' \
      'export TMPDIR="${BOTSTER_MUTANTS_TMPDIR:-/work/target/tmp}"' \
      'mkdir -p "$TMPDIR"' \
      'exec /usr/local/cargo/bin/cargo-mutants "$@"' > /usr/local/botster/bin/cargo-mutants && \
    chmod 755 /usr/local/botster/bin/cargo-mutants

ENV PATH=/usr/local/botster/bin:$PATH \
    CARGO_HOME=/cargo \
    npm_config_cache=/npm \
    CARGO_TERM_COLOR=never \
    TZ=UTC
