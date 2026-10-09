#!/bin/bash
# Fork A6: Mac Zig run. Usage: mac-zig.sh <fork worktree> <log prefix>
# Prints the head, the Zig path, the Build Summary and the exit status of each step.
# The full test output is kept in <log prefix>.test-lib-vt.txt.
set -u
cd "$1" || exit 2
P=$2
ZIG=/Users/jasonconigliari/.local/share/mise/installs/zig/0.16.0/bin/zig
echo "head $(git rev-parse HEAD)"
echo "tracked changes: $(git status --porcelain --untracked-files=no | wc -l | tr -d ' ')"
echo "zig $ZIG $($ZIG version)"
for i in 1 2 3; do
  $ZIG build --fetch=all && break
  echo "fetch attempt $i failed"
done
$ZIG build test-lib-vt --summary all > "$P.test-lib-vt.txt" 2>&1
echo "test-lib-vt exit $?"
grep -E "^Build Summary" "$P.test-lib-vt.txt"
$ZIG build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Demit-xcframework=false > "$P.lib-build.txt" 2>&1
echo "lib build exit $?"
ls -l zig-out/lib/
