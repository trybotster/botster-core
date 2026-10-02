#!/usr/bin/env bash
# Fills the package store of the libghostty-vt build, with network. It is the only step that fetches Zig packages;
# build.rs fetches nothing (see build.rs). Run it once per Ghostty pin, and again when the pin changes its dependencies.
#
# It builds libghostty-vt once with a scratch Zig global cache, so Zig fetches exactly the packages that this build
# needs, and then copies the files that build_data.rs lists into the store (default ~/.cache/botster/zig-packages,
# or $BOTSTER_ZIG_PACKAGES). The store is never written by anything else.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
ghostty="$here/vendor/ghostty"
store="${BOTSTER_ZIG_PACKAGES:-$HOME/.cache/botster/zig-packages}"
zig="${BOTSTER_ZIG:-zig}"

[ -f "$ghostty/build.zig" ] || { echo "prefetch-zig: no Ghostty source at $ghostty (git submodule update --init)" >&2; exit 1; }

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT

# The build arguments and the package list come from build_data.rs, so there is one list.
args=$(sed -n '/pub const GHOSTTY_BUILD_ARGS/,/^];/p' "$here/build_data.rs" | sed -n 's/^ *"\(.*\)",$/\1/p')
packages=$(sed -n '/pub const ZIG_PACKAGES/,/^];/p' "$here/build_data.rs" | sed -n 's/^ *"\(.*\)",$/\1/p')

(cd "$ghostty" && $zig $args --cache-dir "$scratch/local" --global-cache-dir "$scratch/global" --prefix "$scratch/prefix")

mkdir -p "$store/p"
for hash in $packages; do
  [ -f "$scratch/global/p/$hash.tar.gz" ] || { echo "prefetch-zig: the build did not fetch $hash; the pin changed its dependencies, update ZIG_PACKAGES in build_data.rs" >&2; exit 1; }
  cp "$scratch/global/p/$hash.tar.gz" "$store/p/$hash.tar.gz"
done
echo "prefetch-zig: $(echo "$packages" | wc -w | tr -d ' ') packages are in $store"
