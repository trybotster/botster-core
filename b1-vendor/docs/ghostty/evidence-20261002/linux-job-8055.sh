#!/bin/bash
# one-off ghostty upstream-sync job: $1 = snapshot dir, $2 = cpus
set -u
SNAP="$1"; CPUS="$2"; NAME="ghostty-sync-$$"
trap 'docker rm -f "$NAME" >/dev/null 2>&1; rm -rf "$SNAP"' EXIT
IMAGE=botster-core-gate:bbc12a44852f
docker run --rm --name "$NAME" --label testq.job=ghostty-sync --cpus "$CPUS" --memory 16g --init \
  -v "$SNAP":/w -w /w "$IMAGE" bash -c '
set -u
uname -m; zig version; gcc --version | head -1; ldd --version | head -1
export ZIG_GLOBAL_CACHE_DIR=/w/zig-global
ARGS="-Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Demit-xcframework=false"
for v in pre10 new; do
  echo "===== $v: extract"; mkdir -p /w/$v; tar -xf /w/$v.tar -C /w/$v
  cd /w/$v
  echo "===== $v: zig build (Botster args)"
  zig build $ARGS --prefix /w/$v-prefix; echo "build exit $?"
  A=$(find /w/$v-prefix -name "libghostty-vt.a" | head -1); echo "archive: $A"
  echo "===== $v: defined allocator symbols (nm --defined-only)"
  nm -A --defined-only "$A" 2>/dev/null | grep -E " [TtWwVv] (malloc|calloc|realloc|free)$" || echo "(none defined)"
  echo "===== $v: undefined allocator references"
  nm -u "$A" 2>/dev/null | grep -E " (malloc|calloc|realloc|free)$" | sort -u
  echo "===== $v: C link test"
  gcc -O1 -o /w/$v-repro /w/repro.c -I/w/$v-prefix/include "$A" -lm -lpthread && /w/$v-repro; echo "repro exit $?"
done
cd /w/new
echo "===== new: zig build test-lib-vt (full)"
zig build test-lib-vt --summary all 2>&1 | tail -n 40; echo "test-lib-vt exit ${PIPESTATUS[0]}"
'
