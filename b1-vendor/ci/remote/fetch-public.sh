#!/usr/bin/env bash
# Every download of the gate, in a container with network and no token (ci/remote/job-common.sh): cargo's crates into the
# cargo volume, and the npm packages of ts/ into the npm cache (/npm). The gate container then runs with no network
# (CARGO_NET_OFFLINE, npm_config_offline). A Mac gate runs this file in its slot, with the host's network.
set -euo pipefail

cargo fetch --locked -q
# The gate skips its ts step where npm is missing (a Mac node), so this does too.
if command -v npm >/dev/null; then (cd ts && npm ci --no-audit --no-fund >/dev/null); fi
echo "fetch: cargo's crates and the npm packages of ts/ are in the cargo volume and the npm cache"
