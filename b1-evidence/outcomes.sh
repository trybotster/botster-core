# B-1 evidence (Linux): the full strict conformance run, testkit and real tiers, must-pass then pending (--ignored), seeds 0-31, before and after. Prints the diff of the two trees' lines (it must be empty).
set -u
echo "B1 $(git rev-parse HEAD) $(uname -sr)"
export BOTSTER_SEEDS=0-31 BOTSTER_PENDING_STRICT=1
filter() {
  grep -E '^test |^(FAIL|INCONCLUSIVE|UNSUPPORTED_CONTROL|NOT_APPLICABLE) conf::|^ +(expected|received): |^test result|^conformance' \
    | sed -E 's/finished in [0-9.]+s//'
}
run() {
  local tag=$1
  { cargo test -p botster-core --test conformance --no-fail-fast 2>&1 | filter
    cargo test -p botster-core --test conformance --no-fail-fast -- --ignored 2>&1 | filter; } > "/tmp/b1-$tag-testkit.txt"
  { cargo test -p botster-core --features slow --test slow_conformance --no-fail-fast 2>&1 | filter
    cargo test -p botster-core --features slow --test slow_conformance --no-fail-fast -- --ignored 2>&1 | filter; } > "/tmp/b1-$tag-real.txt"
  wc -l /tmp/b1-$tag-*.txt
}
cargo xtask prebuild-worker 2>&1 | tail -1
run before
sh b1-evidence/patch.sh
cargo xtask prebuild-worker 2>&1 | tail -1
run after
for t in testkit real; do
  echo "B1 diff $t:"; diff <(sort "/tmp/b1-before-$t.txt") <(sort "/tmp/b1-after-$t.txt"); echo "B1 diff $t exit: $? (0 = identical)"
  for w in before after; do grep -E "^test result|^conformance" "/tmp/b1-$w-$t.txt" | sed "s/^/B1 $w $t | /"; done
done
echo "B1 end"
