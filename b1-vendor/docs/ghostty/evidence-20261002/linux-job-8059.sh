#!/bin/bash
# Recorded Ghostty sync Linux job: hashes every input before use, then the allocator symbols, the Rust link program
# before and after patch 10, and test-lib-vt at the candidate. $1 = snapshot dir, $2 = cpus.
set -u
SNAP="$1"; CPUS="$2"; NAME="ghostty-sync3-$$"
trap 'docker rm -f "$NAME" >/dev/null 2>&1; rm -rf "$SNAP"' EXIT
echo "testq snapshot: $SNAP"; echo "date: $(date -u +%FT%TZ)"; echo "host: $(uname -srm)"
echo "input sha256 (before extraction):"; (cd "$SNAP" && sha256sum new.tar pre10.tar repro.rs job.sh)
docker run --rm --name "$NAME" --label testq.job=ghostty-sync --cpus "$CPUS" --memory 16g --init --user "$(id -u):$(id -g)" -e HOME=/w \
  -v "$SNAP":/w -w /w botster-core-gate:bbc12a44852f bash -c '
set -u
echo "image tools: zig $(zig version); $(rustc --version); $(ldd --version | head -1)"
echo "input sha256 inside the container:"; sha256sum /w/new.tar /w/pre10.tar /w/repro.rs
export ZIG_GLOBAL_CACHE_DIR=/w/zig-global
ARGS="-Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Demit-xcframework=false"
for v in pre10 new; do
  mkdir -p /w/$v; tar -xf /w/$v.tar -C /w/$v; cd /w/$v
  echo "+ ($v) zig build $ARGS"; zig build $ARGS --prefix /w/$v-prefix >/w/$v-build.log 2>&1; echo "===== $v: build exit $?"
  A=/w/$v-prefix/lib/libghostty-vt.a; echo "archive sha256: $(sha256sum $A | cut -d" " -f1)"
  echo "----- $v defined allocator symbols"; nm -A --defined-only "$A" | grep -E " [TtWwVv] (malloc|calloc|realloc|free)$" || echo "(none defined)"
  echo "----- $v undefined allocator references"; nm -u "$A" | grep -E " (malloc|calloc|realloc|free)$" | sort -u
  for opt in 0 3; do
    rustc -C opt-level=$opt -L native=/w/$v-prefix/lib -o /w/$v-repro-O$opt /w/repro.rs 2>&1 | tail -5
    echo "----- $v rust O$opt"; /w/$v-repro-O$opt; echo "exit $?"
  done
done
cd /w/new
echo "+ (new) zig build test-lib-vt --summary all"
zig build test-lib-vt --summary all > /w/test.log 2>&1; echo "test-lib-vt exit $?"
grep -E "^Build Summary|run test .*(pass|fail)" /w/test.log
'
