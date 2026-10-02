#!/usr/bin/env bash
# The fetch step of a gate job on the shared test host (ci/remote/job.sh). It runs in its own container with the host's
# GitHub token, and downloads every dependency into the cargo volume, so the gate container runs with no network:
#   - the workspace's dependencies, botster-contracts (private) included;
#   - the dependencies of the conformance probe, which `cargo xtask prebuild-worker` installs from the same botster-contracts
#     tag with `cargo install --git … --locked`.
# It runs no code of this repo, only cargo's resolver.
set -euo pipefail

cargo fetch --locked -q

# The botster-contracts commit of Cargo.lock, and cargo's checkout of it.
rev=$(sed -n 's|^source = "git+https://github.com/trybotster/botster-contracts?tag=[^#]*#\([0-9a-f]*\)"$|\1|p' Cargo.lock | sort -u)
[ "$(printf '%s\n' "$rev" | wc -l)" = 1 ] && [ -n "$rev" ] || { echo "fetch.sh: Cargo.lock names no single botster-contracts commit" >&2; exit 1; }
checkout=
for candidate in "$CARGO_HOME"/git/checkouts/botster-contracts-*/"${rev:0:7}"; do
  [ -f "$candidate/Cargo.lock" ] && checkout=$candidate
done
[ -n "$checkout" ] || { echo "fetch.sh: no cargo checkout of botster-contracts $rev" >&2; exit 1; }
cargo fetch --locked -q --manifest-path "$checkout/Cargo.toml"
# `cargo install --git … --tag <tag>` looks the tag up in cargo's git database, under refs/remotes/origin/tags/<tag>; the
# locked workspace fetch stored only the commit. Fetch the tag ref the way cargo would.
tag=$(sed -n 's|^source = "git+https://github.com/trybotster/botster-contracts?tag=\([^#]*\)#.*|\1|p' Cargo.lock | sort -u)
for db in "$CARGO_HOME"/git/db/botster-contracts-*; do
  git -C "$db" fetch -q --no-tags https://github.com/trybotster/botster-contracts "+refs/tags/$tag:refs/remotes/origin/tags/$tag"
done
echo "fetch: dependencies of the workspace and of botster-contracts ${rev:0:8} are in the cargo volume"
