#!/bin/bash
# one-off ghostty upstream-sync job 2: Rust link repro before and after patch 10
set -u
SNAP="$1"; CPUS="$2"; NAME="ghostty-sync2-$$"
trap 'docker rm -f "$NAME" >/dev/null 2>&1; rm -rf "$SNAP"' EXIT
docker run --rm --name "$NAME" --label testq.job=ghostty-sync --cpus "$CPUS" --memory 16g --init --user "$(id -u):$(id -g)" -e HOME=/w \
  -v "$SNAP":/w -w /w botster-core-gate:bbc12a44852f bash -c '
set -u
rustc --version
export ZIG_GLOBAL_CACHE_DIR=/w/zig-global
ARGS="-Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Demit-xcframework=false"
for v in pre10 new; do
  mkdir -p /w/$v; tar -xf /w/$v.tar -C /w/$v; cd /w/$v
  zig build $ARGS --prefix /w/$v-prefix >/w/$v-build.log 2>&1; echo "===== $v: build exit $?"
  for opt in 0 3; do
    rustc -C opt-level=$opt -L native=/w/$v-prefix/lib -o /w/$v-repro-O$opt /w/repro.rs 2>&1 | tail -5
    echo "----- $v rust O$opt"; /w/$v-repro-O$opt; echo "exit $?"
  done
done
cd /w/new
echo "===== new: zig build test-lib-vt (full)"
zig build test-lib-vt --summary all > /w/test.log 2>&1; echo "test-lib-vt exit $?"
grep -E "^Build Summary|run test .*(pass|fail)" /w/test.log
'
