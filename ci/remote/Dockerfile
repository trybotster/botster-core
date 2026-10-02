# Gate image for the shared test host (ci/remote/job.sh; botster-contracts docs/BUILD.md "How each stage runs"). It holds
# only the toolchains and the cargo tools. The source is mounted at run time, and cargo's home, the target directory and
# Zig's cache live in volumes. job.sh tags the image with the hash of this file, rust-toolchain.toml and the pinned
# nightly, so a toolchain change builds a new image.
FROM docker.io/library/debian:bookworm-slim

# The stable channel of rust-toolchain.toml, and the nightly of xtask/src/tools.rs (job.sh passes both).
ARG RUST_TOOLCHAIN
ARG RUST_NIGHTLY
# libghostty builds with Zig (build.zig.zon minimum_zig_version; docs/stage1/libghostty-audit.md).
ARG ZIG_VERSION=0.16.0
ARG ZIG_SHA256=70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00
ARG BUILD_JOBS=4

RUN apt-get update -qq && \
    apt-get install --no-install-recommends -y \
      build-essential ca-certificates curl git libssl-dev pkg-config procps xz-utils && \
    rm -rf /var/lib/apt/lists /var/cache/apt/archives

ENV RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:/usr/local/zig:$PATH

RUN test -n "$RUST_TOOLCHAIN" && test -n "$RUST_NIGHTLY" && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
      CARGO_HOME=/usr/local/cargo sh -s -- -y -q --no-modify-path --profile minimal --default-toolchain none && \
    CARGO_HOME=/usr/local/cargo rustup toolchain install "$RUST_TOOLCHAIN" --profile minimal -c rustfmt -c clippy && \
    CARGO_HOME=/usr/local/cargo rustup toolchain install "$RUST_NIGHTLY" --profile minimal && \
    CARGO_HOME=/usr/local/cargo rustup default "$RUST_TOOLCHAIN"

# The versions of the Mac gate. Tools live in /usr/local/cargo/bin; cargo finds them on PATH while CARGO_HOME points at
# the volume.
RUN CARGO_HOME=/usr/local/cargo CARGO_BUILD_JOBS=$BUILD_JOBS cargo install -q --locked \
      cargo-nextest@0.9.146 cargo-mutants@27.1.0 cargo-bolero@0.13.5 cargo-public-api@0.52.0 && \
    rm -rf /usr/local/cargo/registry /usr/local/cargo/git

RUN curl -fsSL -o /tmp/zig.tar.xz "https://ziglang.org/download/$ZIG_VERSION/zig-x86_64-linux-$ZIG_VERSION.tar.xz" && \
    echo "$ZIG_SHA256  /tmp/zig.tar.xz" | sha256sum -c - && \
    mkdir /usr/local/zig && tar -xJf /tmp/zig.tar.xz -C /usr/local/zig --strip-components=1 && rm /tmp/zig.tar.xz && \
    zig version


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
    ZIG_GLOBAL_CACHE_DIR=/zig \
    CARGO_TERM_COLOR=never \
    TZ=UTC
