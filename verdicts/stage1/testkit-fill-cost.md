# PR #223 — Fill cost and dependency optimization

## Round 1 — 2026-10-10

Reviewed head: `a9d99fee96e3e36a721ece59be84ed432548cc19`.
PR and gate base: `26843c74b460688c8e7219eb48a60420b930061e`.
Tier: HIGH. The PR changes the shared testkit and workspace build configuration.
Scope: all five changed files, their callers, trace consumers, the supplied gate, and timing evidence.

### R1-1 — LOW — The optimization description incorrectly says the dependencies are test-only

The new Cargo.toml comment and PR body describe all four selected packages as test-only checker dependencies.
`botster-route-codec` and `serde_json` are normal dependencies of production crates, including host, core-link, worker-core, and the facade.
Their package overrides also affect production consumers built with the dev profile.
Correct the comment and PR body to describe this scope.

The lead confirmed the broader scope in `msg_plugin-w_1791651808_c21fd8`.
The lead permits these four package overrides, with only opt-level changed and no override for a Core workspace crate.
The descriptions must state that dev/test production dependencies are optimized while assertions and overflow checks retain their defaults.
The reviewed configuration changes only opt-level to 2 for the four named packages.
This finding concerns the description, not the authorized optimization scope.
The package reviewer independently reported the same finding as F98 LOW.
Status: OPEN.

### Source checks

`take_front` copies the requested prefix across the deque's two slices, then drains that prefix.
Both callers retain their existing length bounds. Stream reads still respect descriptor boundaries and update byte counters.
Program reads retain the scheduler's selected size and the atomic-piece boundary.
The wrap proof checks copied bytes, remaining bytes, and an empty read.

The new fill pattern repeats the alphabet and truncates it to the requested length.
Its proofs retain the partial-alphabet case and add exact-alphabet and zero-length cases.
The pattern still starts at `a` for each fill.

An untraced simulation no longer formats input and action values.
The machine still handles each input and performs every action in the same order.
The only existing trace consumers are the simulation tests; their helper enables tracing.
The new proof checks zero formatting calls when tracing is off and six calls plus three entries when tracing is on.
I found no source defect in these changes.

### Timing evidence and limits

I read the saved script and output in `shared/core-stage1/evidence/p3-fill-cost/`.
The scratch head is `8bd0e6b45cb0a027a736afb5837fcf92a7a2ff0c`, over the v0.1.25 pin at `475372d1`.
The four changed testkit files are byte-identical to the reviewed head.
The complete five-file optimization diff matches after removal of index and hunk metadata.
The saved script matches the scratch commit's script.

The script runs `ou_3_progressing_reader_lossless` for seeds 0–31 three times per configuration on gaming.
All twelve runs report one passing trial.
Observed times are 12.93–12.98 seconds without either change, 10.92–10.93 with A, 4.19–4.20 with B, and 2.10–2.11 with both.
The two clean workspace test builds measure A-only at 75.419 seconds and A+B at 77.208 seconds.
Those builds measure B's added build cost with A held constant; they do not measure a clean build without A.
The job exits zero after 287 seconds.

A+B still exceeds the two-second limit on this node.
The PR body states that limitation and assigns the remaining work to contracts change C.
This review does not approve C or establish that the v0.1.25 transcript meets the default-tier time limit.
The cross-node gate comparison is not a controlled timing comparison; the PR body identifies that limit.

### Exact-head gate and verdict

Gate: `botster-core-stage1-testkit-fill-cost-a9d99fee-pool-20261010-093819-57747.log`.
The header names the reviewed head and base. All ten stages pass in 372.0 seconds.
Default tests: 1,481 passed. Slow tests: 378 passed.
The new wrap and trace proofs and the fill-pattern proof have PASS lines.
Conformance remains 193 testkit passes, 407 pending trials, 70 entries without transcripts, two deferred trials, and 18 withdrawn trials.
Minimum counts remain testkit 50/69, real-passing 29/68, and real-accepted 29/69.
The report retains 87 pending-real ids with zero passes.
Both mutation stages report 21 mutants: 17 caught, four unviable, zero missed, and zero timeouts.
The second stage takes 120.4 seconds and still does not enable the slow feature.
The pool job exits zero after 503 seconds on gaming; the wrapper exits zero after 504 seconds.

The base is an ancestor of the reviewed head. The merge has no combined diff, and `git diff --check` passes.
The remote base matches. The branch has advanced to `53b6999ff07e92e4083c57c0c7318ed3f04c1d3b`, which awaits replacement READY.
I sent R1-1 to the implementer and package reviewer. The package artifact is pending.
I changed no product code and ran no builds, tests, gates, or timing jobs.

VERDICT: NOT CLEAN (1 LOW open).

## Round 2 — 2026-10-10

Reviewed head: `190c12a36d7ee0acf96a44f4554a93ffd7ee7e16`.
PR and gate base: `150a2069428173c97892160b73e94993233f413a`.
Prior reviewed head: `a9d99fee96e3e36a721ece59be84ed432548cc19`.
Tier: HIGH. The complete round 1 source review remains part of this verdict.

### R1-1 — LOW — CLOSED

The Cargo.toml comment and PR body now describe the production use of the optimized packages.
Both name botster-route-codec and serde_json as production dependencies of Core crates.
Both describe regex-automata's build-script use through bindgen and libproc.
I confirmed that dependency chain in Cargo.lock and the macOS libproc dependency of botster-core-sys.
Both identify botster-hub-conformance as a testkit and xtask dependency.

The same four overrides retain opt-level 2.
Each now explicitly sets debug-assertions=true and overflow-checks=true, preserving the existing dev-profile settings.
No Core workspace crate has an override. These changes satisfy the lead's authorized scope.

### Source and evidence

The four changed testkit files are byte-identical to the round 1 head.
The replacement changes only the Cargo.toml comment and explicit check settings within this PR's own changes.
The new base adds #224's shared payload guard fix, which I separately reviewed as CLEAN.
The merge has no combined diff. The base is an ancestor, and `git diff --check` passes.
The remote implementation head and base match the reviewed commits.

Gate: `botster-core-stage1-testkit-fill-cost-190c12a3-pool-20261010-102544-51971.log`.
The header names the reviewed head and base. All ten stages pass in 372.0 seconds.
Default tests: 1,481 passed. Slow tests: 381 passed.
The wrapped-copy and trace-formatting proofs have PASS lines.
Conformance remains 193 testkit passes, 407 pending trials, 70 entries without transcripts, two deferred trials, and 18 withdrawn trials.
Minimum counts remain testkit 50/69, real-passing 29/68, and real-accepted 29/69.
The report retains 87 pending-real ids with zero passes.
Both mutation stages report 21 mutants: 17 caught, four unviable, zero missed, and zero timeouts.
The second stage takes 119.8 seconds. Its environment setting does not enable the slow feature.
The job and wrapper exit zero after 503 seconds on gaming, with zero queue time.

The round 1 timing evidence retains its scope and limitations.
A+B alone still measures 2.10–2.11 seconds on gaming at v0.1.25.
That evidence does not establish compliance with the two-second limit or approve the future v0.1.26 pin.
The separate contracts C change and the final Core pin gate remain necessary for that conclusion.

The package verdict is CLEAN at `1c09a150e3fb992bfcde328a34ac6c0867cf8fe1`, in `verdicts/p3-worker.md`, round 149.
I read that verdict and confirmed its agreement with the source and supplied evidence.
No integration finding remains. I changed no product code and ran no builds, tests, gates, or measurements.

VERDICT: CLEAN
