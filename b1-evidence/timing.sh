# B-1 evidence (Mac): conf::in_9_legacy_keys_table at seeds 0-31 on the testkit, before (contracts .43 as pinned) and
# after (the vendored .43 + B-1): 10 alternating timed runs per tree, then 3 whole-run `sample` profiles per tree.
set -u
echo "B1 $(git rev-parse HEAD) $(uname -sr)"
export CARGO_MANIFEST_DIR="$PWD/crates/botster-core" BOTSTER_ONLY=conf::in_9_legacy_keys_table BOTSTER_SEEDS=0-31 BOTSTER_PENDING_STRICT=1
bin() {
  cargo test -p botster-core --test conformance --no-run --message-format=json 2>/tmp/b1-build.err \
    | python3 -c 'import sys,json
for l in sys.stdin:
    m=json.loads(l)
    if m.get("reason")=="compiler-artifact" and m.get("executable") and m["target"]["name"]=="conformance": print(m["executable"])' | tail -1
}
# Builds both binaries first, then alternates the timed runs, so that load on the node falls on both trees alike.
cargo xtask prebuild-worker 2>&1 | tail -1
before=$(bin); echo "B1 before binary $before"
sh b1-evidence/patch.sh > /dev/null
after=$(bin); echo "B1 after binary $after"
[ -n "$before" ] && [ -n "$after" ] || { tail -30 /tmp/b1-build.err | sed "s/^/B1 build | /"; exit 1; }
timed() {
  local tag=$1 exe=$2 i=$3
  /usr/bin/time -p "$exe" > "/tmp/b1-$tag-run$i.txt" 2>&1
  grep -q '^test result: ok. 1 passed' "/tmp/b1-$tag-run$i.txt" || tail -15 "/tmp/b1-$tag-run$i.txt" | sed "s/^/B1 $tag run$i output | /"
  echo "B1 $tag run$i | $(grep '^real' "/tmp/b1-$tag-run$i.txt")"
}
for i in 1 2 3 4 5 6 7 8 9 10; do timed before "$before" $i; timed after "$after" $i; done
for t in before after; do
  echo "B1 $t median real: $(grep -h '^real' /tmp/b1-$t-run*.txt | awk '{print $2}' | sort -n | sed -n '5,6p' | paste -sd' ' -)"
done
# Three whole-run profiles per tree: `sample` attaches at once and stops when the run ends. The frames are the sums of
# the three demangled profiles.
filt=$(command -v rustfilt || echo "$HOME/.cargo/bin/rustfilt")
profile() {
  local tag=$1 exe=$2
  rm -f /tmp/b1-$tag-*.sample.txt /tmp/b1-$tag-*.dm.txt
  for i in 1 2 3; do
    "$exe" > /dev/null 2>&1 &
    local pid=$!
    sample "$pid" 30 -file "/tmp/b1-$tag-$i.sample.txt" > /dev/null 2>&1
    wait "$pid"
    "$filt" -i "/tmp/b1-$tag-$i.sample.txt" -o "/tmp/b1-$tag-$i.dm.txt"
  done
  python3 - "$tag" /tmp/b1-$tag-*.dm.txt <<'PY'
import fileinput, re, sys
want = ["CoreDriver as botster_conformance::driver::StepDriver>::exec ", "CoreDriver>::exec_step ", "core::clone::Clone>::clone",
        "clone_subtree", "::input_log_track", "matcher::matches", "::pump_until", "::next_event", "thread_start"]
total = {w: 0 for w in want}
stack = []
direct = 0
for line in fileinput.input(sys.argv[2:], errors="replace"):
    m = re.match(r"^([ +!:|]*)(\d+) (.*?)  \(in ", line)
    if not m: continue
    depth, count, name = len(m.group(1)), int(m.group(2)), m.group(3) + " "
    while stack and stack[-1][0] >= depth: stack.pop()
    for w in want:
        if w in name and not any(w in n for _, n in stack): total[w] += count
    if "core::clone::Clone>::clone " in name and stack and want[0] in stack[-1][1]: direct += count
    stack.append((depth, name))
print(f"B1 {sys.argv[1]} frame | {direct:6d} clone called by exec itself (the bindings copy)")
for w in want: print(f"B1 {sys.argv[1]} frame | {total[w]:6d} {w.strip()}")
PY
}
profile before "$before"
profile after "$after"
echo "B1 end"
