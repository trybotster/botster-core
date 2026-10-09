#!/bin/bash
# Fork A6 sync of 2026-10-09: the evidence run inside a botster-core gate tree, from its root:
#   botster-gate --on mac|linux <core worktree> -- bash docs/stage1/ghostty-upstream-sync-20261009/evidence.sh
# It prints the heads, the Zig version, then for each step its exit status:
#   1. the lib-vt build with the binding's GHOSTTY_BUILD_ARGS and an EMPTY Zig global cache, and the packages that the
#      build fetched, compared with build_data.rs ZIG_PACKAGES (needs network; it says so when it has none);
#   2. zig build test-lib-vt --summary all (its Build Summary line);
#   3. the binding tests, cargo nextest run -p botster-terminal-ghostty.
set -u
g=crates/botster-terminal-ghostty
f=$g/vendor/ghostty
zig=${BOTSTER_ZIG:-zig}
echo "core head $(git rev-parse HEAD), tracked changes $(git status --porcelain --untracked-files=no | wc -l | tr -d ' ')"
echo "fork head $(git -C "$f" rev-parse HEAD), tracked changes $(git -C "$f" status --porcelain --untracked-files=no | wc -l | tr -d ' ')"
echo "zig $(command -v "$zig") $("$zig" version)"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
args=$(sed -n '/pub const GHOSTTY_BUILD_ARGS/,/^];/p' "$g/build_data.rs" | sed -n 's/^ *"\(.*\)",$/\1/p')
packages=$(sed -n '/pub const ZIG_PACKAGES: /,/^];/p' "$g/build_data.rs" | sed -n 's/^ *"\(.*\)",$/\1/p' | sort)

echo "== 1. lib-vt build, empty global cache: zig $(echo $args)"
(cd "$f" && "$zig" $args --cache-dir "$scratch/local" --global-cache-dir "$scratch/global" --prefix "$scratch/prefix") > "$scratch/lib.txt" 2>&1
echo "lib build exit $?"
tail -5 "$scratch/lib.txt"
ls -l "$scratch/prefix/lib" 2>&1
fetched=$(ls "$scratch/global/p" 2>/dev/null | sed -n 's/\.tar\.gz$//p' | sort)
echo "fetched packages ($(echo "$fetched" | grep -c .)):"; echo "$fetched"
if [ "$fetched" = "$packages" ]; then echo "package list: SAME as build_data.rs ZIG_PACKAGES"; else
  echo "package list: DIFFERENT from build_data.rs ZIG_PACKAGES:"; diff <(echo "$packages") <(echo "$fetched"); fi

echo "== 2. zig build test-lib-vt --summary all"
if [ -z "$fetched" ]; then
  # No network: the tests use the gate's package store (layout p/<hash>.tar.gz, as a Zig global cache).
  store=${BOTSTER_ZIG_PACKAGES:-$HOME/.cache/botster/zig-packages}
  mkdir -p "$scratch/global/p" && cp "$store"/p/*.tar.gz "$scratch/global/p/" && echo "global cache seeded from $store"
else
  # The tests need more packages than the library. The Mac Zig fetch can fail with TlsInitializationFailed; it is tried
  # up to three times, as the earlier record's mac-zig.sh did.
  for i in 1 2 3; do
    (cd "$f" && "$zig" build --fetch=all --cache-dir "$scratch/local" --global-cache-dir "$scratch/global") > "$scratch/fetch.txt" 2>&1 && break
    echo "fetch attempt $i failed: $(grep -m1 -o 'error: .*' "$scratch/fetch.txt")"
  done
fi
(cd "$f" && "$zig" build test-lib-vt --summary all --cache-dir "$scratch/local" --global-cache-dir "$scratch/global") > "$scratch/test.txt" 2>&1
echo "test-lib-vt exit $?"
grep -E "^Build Summary" "$scratch/test.txt" || tail -40 "$scratch/test.txt"

echo "== 3. cargo nextest run -p botster-terminal-ghostty"
env -u RUSTUP_TOOLCHAIN CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo nextest run -p botster-terminal-ghostty > "$scratch/binding.txt" 2>&1
echo "binding tests exit $?"
grep -E "Summary|FAIL" "$scratch/binding.txt" | tail -20
