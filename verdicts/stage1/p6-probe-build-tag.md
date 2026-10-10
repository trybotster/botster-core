# PR #235 — Probe build directory by contracts tag

## Round 1 — 2026-10-10

Reviewed head: `d76443c26368b356630dd989466d73e8d4699bc8`.
PR and full gate base: `89d57cc7d07957b88003722897cf43e540d19bcc`.
Tier: HIGH under BUILD rule 1 for gate infrastructure and rule 3 for workspace configuration.
Scope: the complete two-file delta, affected prebuild path, exclusion reason, PR body, and supplied evidence.
The lead orders this PR before #232.
The Git merge base equals the recorded gate base.

`build_probe` reads the same contracts git URL and tag from Cargo.toml through `contracts_source`.
It now passes `probe_build_dir(meta.target_dir, tag)` as cargo install's target directory.
The pure decision returns `<target>/probe-build/<tag>`.
Different named contracts tags therefore use different build directories. Repeated builds of the same tag retain their cache.
The source no longer uses the old unqualified probe-build directory.
The PR records the retained directories of older tags as its storage tradeoff.

The install root remains probe-install. Cargo install still uses --locked and --force with the recorded git URL and tag.
The existing capped command still checks the process exit status before copying the installed binary.
The candidate replacement still uses a new file, and its SHA256 still enters the three-binary manifest.
The command removes the prior manifest before building and writes its replacement only after all builds succeed.
The change adds no new build in a test and changes no production process or platform-specific implementation.

The new unit proof checks the exact directory for contracts-v0.1.27 and distinct directories for v0.1.26 and v0.1.27.
It tests the cache identity decision, not a private queue or an internal counter.
The exact-head gate records this proof and the contracts-source proof as PASS.

The new mutation regex covers only `build_probe -> Result<(String, String)>` whole-body Ok replacements in prebuild.rs.
It covers the four body replacements reported by the first gate.
Static regex checks confirm that it does not cover `probe_build_dir` or `contracts_source`.
The reason cites both decisions with their named proofs.
I read both decision bodies, both proofs, and the complete build_probe body.
The body forwards source and path selection to those decisions and performs build, copy, and hash I/O.
No broader file, crate, operator, or decision exclusion is added.
The final mutation run still executes and catches the new path decision's mutant.

The PR changes no contract pin, dependency, pending ID, conformance count, guard, or test-process owner.
The two commits descend directly from the current v1 base. No base merge is required at this head.
The remote PR and branch heads and v1 base match the recorded commits. `git diff --check` passes.

### Supplied cache evidence

I read the old-path failures in these logs:

- `~/botster-sessions/gates/botster-core-stage1-p6-pty-order-ca8f06fb-pool-20261010-153533-37614.log`.
- `~/botster-sessions/gates/botster-core-stage1-p4a-route-input-d1d8742a-pool-20261010-153548-38719.log`.

Both fail to import botster_probe_script::control while building the v0.1.27 probe.
Neither records a botster-probe-script compilation before that failure.
Both failed commands name the old unqualified target/probe-build directory.
These are supplied failures, not reviewer reproductions of Cargo's cache behavior.

The first changed-path gate uses `de631fe4387092ef63f9f96051a30f34c5b87c89` and the same v1 base.
Log: `~/botster-sessions/gates/botster-core-stage1-p6-probe-build-tag-de631fe4-pool-20261010-153900-52805.log`.
It records compilation of botster-probe-script from contracts b161db8 and a passing prebuild-worker step.
All jobs except mutations pass. Four build_probe body replacements survive both tiers; the path decision's mutant is caught.
The final commit adds the reviewed exclusion. Its entire prebuild.rs equals the first gate's source.

### Exact-head gate

Log: `~/botster-sessions/gates/botster-core-stage1-p6-probe-build-tag-d76443c2-pool-20261010-155829-26279.log`.
Its header names the exact head and base. The command is plain cargo xtask ci through the pool.
All ten jobs pass. Prebuild installs contracts-v0.1.27 at b161db87 and writes the three-binary candidate manifest.
The build reuses the directory already built for that same tag.
Default: 1499 passing tests, 17.139 seconds. Slow: 384 passing tests, 103.256 seconds.
Both conformance reports show 193 passed and zero failed.
Strict coverage remains 386/407, with 21 real-only transcripts and two held transcripts.
Minimum counts remain testkit 50/69, real 29/68, and real-accepted 29/69.
Real-pending remains 87, with zero newly passing IDs.
Stage 1 has one mutant, caught, with zero missed, timeout, or unviable, in 81.4 seconds.
Stage 2 does not run because stage 1 misses none.
The job exits zero after 307 seconds. The wrapper exits zero after 309 seconds.

I found no source defect in this scope.
I read package CLEAN at `f4d62423109bf2e2acbc7a8848c6e5fc134ecfbf`, `verdicts/p6-probe-build-tag.md`, Round 1.
The package verdict covers this exact head and base. Both reviews agree on the full delta, exclusion, and supplied evidence.
I changed no product code and ran no build, test, gate, mutation job, merge-check job, or reversal job.

VERDICT: CLEAN (0 open).
