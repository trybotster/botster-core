#!/bin/bash
# Fork A6 sync of 2026-10-09: the evidence run inside a botster-core gate tree, from its root:
#   botster-gate --on mac|linux <core worktree> -- bash docs/stage1/ghostty-upstream-sync-20261009/evidence.sh
# It prints the heads, the Zig version, then for each step its exit status:
#   1. the lib-vt build with the binding's GHOSTTY_BUILD_ARGS and an EMPTY Zig global cache, and the packages that the
#      build fetched, compared with build_data.rs ZIG_PACKAGES (needs network; it says so when it has none; the gate's
#      binding build of step 3 covers the build from the package store);
#   2a. zig build test-lib-vt --summary all in upstream's default configuration (SIMD, the app packages); it needs
#       network to fetch the test packages, and without network it says so and does not run;
#   2b. zig build test-lib-vt --summary all with the shipped options: GHOSTTY_BUILD_ARGS without "build" and without
#       -Doptimize, so the tests run in Debug with the safety checks; its global cache is seeded only with the packages
#       of build_data.rs ZIG_PACKAGES, from the gate's package store, so the step also shows that the list is sufficient;
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

# Zig 0.16 can also take packages from the project-local zig-pkg directory of the fork tree (untracked), for example
# after a prefetch in the same tree. Then the "empty-cache" build is not empty, so the script reports the directory.
if [ -e "$f/zig-pkg" ]; then echo "project-local $f/zig-pkg: PRESENT ($(ls "$f/zig-pkg" | wc -l | tr -d ' ') entries)"; else
  echo "project-local $f/zig-pkg: absent"; fi

echo "== 1. lib-vt build, empty global cache: zig $(echo $args)"
(cd "$f" && "$zig" $args --cache-dir "$scratch/local" --global-cache-dir "$scratch/global" --prefix "$scratch/prefix") > "$scratch/lib.txt" 2>&1
status=$?
nonet=
if [ $status -ne 0 ] && grep -q "unable to connect to server" "$scratch/lib.txt"; then
  nonet=1
  echo "lib build exit $status: NOT RUN, no network (the empty-cache check fetches every package)"
else
  echo "lib build exit $status"
fi
tail -5 "$scratch/lib.txt"
ls -l "$scratch/prefix/lib" 2>&1
fetched=$(ls "$scratch/global/p" 2>/dev/null | sed -n 's/\.tar\.gz$//p' | sort)
echo "fetched packages ($(echo "$fetched" | grep -c .)):"; echo "$fetched"
if [ -n "$nonet" ]; then echo "package list: not compared (no network)"
elif [ "$fetched" = "$packages" ]; then echo "package list: SAME as build_data.rs ZIG_PACKAGES"; else
  echo "package list: DIFFERENT from build_data.rs ZIG_PACKAGES:"; diff <(echo "$packages") <(echo "$fetched"); fi

echo "== 2a. zig build test-lib-vt --summary all, upstream default configuration"
if [ -n "$nonet" ]; then
  echo "test-lib-vt default: NOT RUN, no network (the test packages are not in the package store)"
else
  # The tests need more packages than the library. The Mac Zig fetch can fail with TlsInitializationFailed; it is tried
  # up to three times, as the earlier record's mac-zig.sh did.
  for i in 1 2 3; do
    (cd "$f" && "$zig" build --fetch=all --cache-dir "$scratch/local" --global-cache-dir "$scratch/global") > "$scratch/fetch.txt" 2>&1 && break
    echo "fetch attempt $i failed: $(grep -m1 -o 'error: .*' "$scratch/fetch.txt")"
  done
  (cd "$f" && "$zig" build test-lib-vt --summary all --cache-dir "$scratch/local" --global-cache-dir "$scratch/global") > "$scratch/test.txt" 2>&1
  echo "test-lib-vt default exit $?"
  grep -E "^Build Summary" "$scratch/test.txt" || tail -40 "$scratch/test.txt"
fi

options=$(echo "$args" | grep -v -e '^build$' -e '^-Doptimize=')
store=${BOTSTER_ZIG_PACKAGES:-$HOME/.cache/botster/zig-packages}
echo "== 2b. zig build test-lib-vt --summary all $(echo $options), Debug, global cache seeded with ZIG_PACKAGES from $store"
mkdir -p "$scratch/shipped/p"
for hash in $packages; do cp "$store/p/$hash.tar.gz" "$scratch/shipped/p/" || echo "seed: $hash MISSING from $store"; done
echo "seeded $(ls "$scratch/shipped/p" | wc -l | tr -d ' ') packages"
(cd "$f" && "$zig" build test-lib-vt --summary all $options --cache-dir "$scratch/local-shipped" --global-cache-dir "$scratch/shipped") > "$scratch/test-shipped.txt" 2>&1
echo "test-lib-vt shipped exit $?"
grep -E "^Build Summary" "$scratch/test-shipped.txt" || tail -40 "$scratch/test-shipped.txt"
extra=$(ls "$scratch/shipped/p" | sed -n 's/\.tar\.gz$//p' | sort | comm -13 <(echo "$packages") -)
if [ -z "$extra" ]; then echo "shipped test packages: none fetched, ZIG_PACKAGES is sufficient"; else
  echo "shipped test packages: FETCHED outside ZIG_PACKAGES:"; echo "$extra"; fi

echo "== 3. cargo nextest run -p botster-terminal-ghostty"
env -u RUSTUP_TOOLCHAIN CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo nextest run -p botster-terminal-ghostty > "$scratch/binding.txt" 2>&1
echo "binding tests exit $?"
grep -E "Summary|FAIL" "$scratch/binding.txt" | tail -20
