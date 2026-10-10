# P6 review: probe build directory per contracts tag

## Round 1 — 2026-10-10

PR: https://github.com/trybotster/botster-core/pull/235.
Exact head: `d76443c26368b356630dd989466d73e8d4699bc8`.
Review and full gate base: `89d57cc7d07957b88003722897cf43e540d19bcc`.
Risk tier: HIGH. The PR changes gate infrastructure and a mutation exclusion.

The reviewer read the full two-file diff, all of `xtask/src/prebuild.rs`, and the current PR body.
The Git merge base matches the recorded gate base.
No package finding remains.

`build_probe` passes `probe_build_dir(meta.target_dir, tag)` as the install command's `--target-dir`.
The helper returns `<target>/probe-build/<contracts tag>`.
Different contracts tags therefore use different build directories.
The install root, `--locked`, `--force`, source Git URL, and source tag remain unchanged.
The candidate installation still replaces the binary through a fresh file and records its SHA256.
The manifest is still written after all installations succeed.

The new proof checks the returned directory for `contracts-v0.1.27` and distinct directories for two tags.
It asserts the path decision's output, with no process or private mutable state.
The PR adds no real-process test and changes no migrated test's contract assertion.
It adds no wait, deadline, sleep, polling loop, group owner, or production reaper.

The Prior art note reuses the existing Cargo target directory and install root.
It adds the tag to the existing target path and introduces no new build component.
The stated tradeoff is retained build data for each tag.

The new exclusion matches only whole-body `build_probe -> Result<(String, String)>` replacements with `Ok(...)`.
`build_probe` performs installation, file reads, candidate replacement, and hashing.
The reason cites the tested `contracts_source` and `probe_build_dir` decisions.
Those decisions remain subject to mutation testing.
The first gate tests four whole-body replacements and misses them in both stages.
The final exclusion does not cover the new directory decision or other mutants of the installation function.

The reviewer read the old-path failure:
`~/botster-sessions/gates/botster-core-stage1-p6-pty-order-ca8f06fb-pool-20261010-153533-37614.log`.
The probe compilation fails on `botster_probe_script::control`.
Its failed command uses `/work/target/probe-build` for the `contracts-v0.1.27` install.

The reviewer read the first new-directory gate:
`~/botster-sessions/gates/botster-core-stage1-p6-probe-build-tag-de631fe4-pool-20261010-153900-52805.log`.
It compiles `botster-probe-script` from the `b161db8` checkout and passes prebuild-worker.
Its four whole-body installation mutants cause the later mutation failure.
Its `prebuild.rs` is identical to the reviewed head. Only the exclusion file changes between those heads.
Both gates use the same named target volume.

The reviewer read the exact-head Linux gate:
`~/botster-sessions/gates/botster-core-stage1-p6-probe-build-tag-d76443c2-pool-20261010-155829-26279.log`.
Its header names the full head and base. All ten jobs pass. The gate exits 0 after 309 seconds.
The probe install reuses the same tag's build and replaces the installed probe.
Prebuild-worker writes the candidate manifest for three binaries.
Default: 1499 passing tests. Slow: 384 passing tests.
The new directory proof passes.
Stage 1 reports one mutant caught, zero missed, zero timeout, and zero unviable, in 81.4 seconds.
Stage 2 does not run.
Both conformance reports show 193 passed and zero failed.
Strict coverage remains 386/407, with 21 real-only transcripts and two held transcripts.
Minimum counts remain testkit 50/69, real 29/68, and real-accepted 29/69.
Pending-real remains 87, with zero newly passing ids.

The reviewer changed no product code and ran no gate, build, test, mutation job, merge check, or reversal.

VERDICT: CLEAN
