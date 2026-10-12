#!/bin/bash
# Recorded Mac run of the Ghostty sync checks. $1 = output prefix dir.
set -u
P="$1"; cd ~/botster-sessions/git-github.com-trybotster-ghostty-botster-upstream-sync-20261002
echo "date: $(date -u +%FT%TZ)"; echo "host: $(uname -srm)"; echo "head: $(git rev-parse HEAD)"
echo "tracked changes: $(git status --porcelain --untracked-files=no | wc -l | tr -d ' ')"
echo "zig: $(command -v zig) $(zig version)"
run() { echo "+ $*"; "$@"; echo "exit $?"; }
run zig build test-lib-vt --summary all --global-cache-dir "$HOME/.cache/botster/zig-global"
rm -rf "$P/prefix-rec"
run zig build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Demit-xcframework=false --prefix "$P/prefix-rec" --global-cache-dir "$HOME/.cache/botster/zig-global"
ls -l "$P/prefix-rec/lib"
