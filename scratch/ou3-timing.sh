#!/usr/bin/env bash
# Scratch evidence only (never merged): the pool time of conf::ou_3_progressing_reader_lossless over seeds 0-31 at contracts
# v0.1.25, and the clean workspace test build, with the fill-cost changes A (testkit) and B (dev-profile opt-level of the four
# checker crates) on and off. "Off" takes the file from 475372d1 (the P6 v0.1.25 pin, which has neither).
set -u
BEFORE=475372d1
ID=conf::ou_3_progressing_reader_lossless
TIMEFORMAT='%R'
echo "head $(git rev-parse HEAD), before $BEFORE, node $(hostname), nproc $(nproc), CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-unset}"

build() { # $1 label, $2 target dir: a clean build of every workspace test target
  rm -rf "$2"
  echo "== $1: clean build (cargo test --workspace --no-run) into $2"
  local t
  t=$( { time CARGO_TARGET_DIR="$2" cargo test --workspace --no-run -q >/dev/null 2>"$2.build.err"; } 2>&1 ) || { echo "BUILD FAILED"; tail -30 "$2.build.err"; exit 1; }
  echo "$1 build_seconds $t"
}

ou3() { # $1 label, $2 target dir: three nextest runs of ou_3 over seeds 0-31
  for i in 1 2 3; do
    CARGO_TARGET_DIR="$2" BOTSTER_SEEDS=0-31 cargo nextest run -p botster-core --test conformance \
      -E "test(=$ID)" --no-fail-fast 2>&1 | grep -E "PASS|FAIL|SIGSEGV|TIMEOUT|error" | grep -v "^\s*Summary" | sed "s/^/$1 run $i: /"
  done
}

T1=/tmp/p3-ou3-ab
T2=/tmp/p3-ou3-a
build "A+B" "$T1"
ou3 "A+B" "$T1"

git checkout "$BEFORE" -- Cargo.toml
build "A only (B off)" "$T2"
ou3 "A only (B off)" "$T2"

git checkout "$BEFORE" -- crates/botster-core-testkit
ou3 "neither (before)" "$T2"

git checkout HEAD -- Cargo.toml
ou3 "B only (A off)" "$T1"

git checkout HEAD -- .
git status --short
rm -rf "$T1" "$T2"
echo done
