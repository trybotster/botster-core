# B-1 evidence (Mac): conf::in_9_legacy_keys_table at seeds 0-31 on the testkit, 3 timed runs and one `sample` profile, before
# (contracts .43 as pinned) and after (the vendored .43 + B-1). Prints the times and the checker frames of each profile.
set -u
echo "B1 $(git rev-parse HEAD) $(uname -sr)"
export BOTSTER_ONLY=conf::in_9_legacy_keys_table BOTSTER_SEEDS=0-31 BOTSTER_PENDING_STRICT=1
bin() {
  cargo test -p botster-core --test conformance --no-run --message-format=json 2>/dev/null \
    | python3 -c 'import sys,json
for l in sys.stdin:
    m=json.loads(l)
    if m.get("reason")=="compiler-artifact" and m.get("executable") and m["target"]["name"]=="conformance": print(m["executable"])' | tail -1
}
measure() {
  local tag=$1 exe
  exe=$(bin); echo "B1 $tag binary $exe"
  for i in 1 2 3; do
    /usr/bin/time -p "$exe" --format terse 2>&1 | grep -E '^(real|test result|FAIL)' | sed "s/^/B1 $tag run$i | /"
  done
  "$exe" --format terse > /dev/null 2>&1 &
  local pid=$!
  sleep 0.2
  sample "$pid" 30 -file "/tmp/b1-$tag.sample.txt" > /dev/null 2>&1
  wait "$pid"
  echo "B1 $tag sample: $(wc -l < /tmp/b1-$tag.sample.txt) lines"
  grep -E 'CoreDriver|::exec |::exec_step|clone_subtree|input_log_track|matcher::matches|pump_until|next_event' "/tmp/b1-$tag.sample.txt" \
    | sed -E 's/^[ +!:|]*//' | sort | uniq -c | sort -rn | head -12 | sed "s/^/B1 $tag frame | /"
}
cargo xtask prebuild-worker 2>&1 | tail -1
measure before
sh b1-evidence/patch.sh
measure after
echo "B1 end"
