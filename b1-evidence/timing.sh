# B-1 evidence (Mac): conf::in_9_legacy_keys_table at seeds 0-31 on the testkit, 3 timed runs and one `sample` profile, before
# (contracts .43 as pinned) and after (the vendored .43 + B-1). Prints the times and the checker frames of each profile.
set -u
echo "B1 $(git rev-parse HEAD) $(uname -sr)"
export BOTSTER_ONLY=conf::in_9_legacy_keys_table BOTSTER_SEEDS=0-31 BOTSTER_PENDING_STRICT=1
bin() {
  cargo test -p botster-core --test conformance --no-run --message-format=json 2>/tmp/b1-build.err \
    | python3 -c 'import sys,json
for l in sys.stdin:
    m=json.loads(l)
    if m.get("reason")=="compiler-artifact" and m.get("executable") and m["target"]["name"]=="conformance": print(m["executable"])' | tail -1
}
measure() {
  local tag=$1 exe
  exe=$(bin); echo "B1 $tag binary $exe"; [ -n "$exe" ] || { tail -30 /tmp/b1-build.err | sed "s/^/B1 $tag build | /"; return; }
  for i in 1 2 3; do
    /usr/bin/time -p "$exe" > "/tmp/b1-$tag-run$i.txt" 2>&1
    grep -E '^(real|test result|FAIL)' "/tmp/b1-$tag-run$i.txt" | sed "s/^/B1 $tag run$i | /"
    grep -q '^test result' "/tmp/b1-$tag-run$i.txt" || tail -15 "/tmp/b1-$tag-run$i.txt" | sed "s/^/B1 $tag run$i output | /"
  done
  "$exe" > /dev/null 2>&1 &
  local pid=$!
  sleep 0.2
  sample "$pid" 4 -file "/tmp/b1-$tag.sample.txt" > /dev/null 2>&1
  wait "$pid"
  echo "B1 $tag sample: $(wc -l < /tmp/b1-$tag.sample.txt) lines"
  # The inclusive samples of each checker frame: the sum of the counts of its call-graph nodes that have no ancestor of
  # the same frame (so a recursive frame counts once).
  python3 - "/tmp/b1-$tag.sample.txt" "$tag" <<'PY'
import re, sys
want = ["CoreDriver as botster_conformance::driver::StepDriver>::exec ", "CoreDriver>::exec_step ", "core::clone::Clone>::clone",
        "clone_subtree", "::input_log_track", "matcher::matches", "::pump_until", "::next_event", "thread_start"]
total = {w: 0 for w in want}
stack = []
direct = 0
for line in open(sys.argv[1], errors="replace"):
    m = re.match(r"^([ +!:|]*)(\d+) (.*?)  \(in ", line)
    if not m: continue
    depth, count, name = len(m.group(1)), int(m.group(2)), m.group(3) + " "
    while stack and stack[-1][0] >= depth: stack.pop()
    for w in want:
        if w in name and not any(w in n for _, n in stack): total[w] += count
    if "core::clone::Clone>::clone " in name and stack and want[0] in stack[-1][1]: direct += count
    stack.append((depth, name))
print(f"B1 {sys.argv[2]} frame | {direct:6d} clone called by exec itself (the bindings copy)")
for w in want: print(f"B1 {sys.argv[2]} frame | {total[w]:6d} {w.strip()}")
PY
}
cargo xtask prebuild-worker 2>&1 | tail -1
measure before
sh b1-evidence/patch.sh
measure after
echo "B1 end"
