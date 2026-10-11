# Contracts #63: integration review (p6/job-network-none)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
PR: https://github.com/trybotster/botster-contracts/pull/63 (P6, `sess-1791518250-0169-fa157d48c599262bb69c87d0b928c13e`). Package reviewer: 016c. Tier: HIGH (BUILD.md risk tiers rule 1: CI steps).

## Round 1: head `16fdb77854c1fd3859e4645797c927dfa1a8195f`

VERDICT: CLEAN (0 open) on this exact head. Merge-ready from the integration review, by fast-forward.

### What I checked and found correct

1. **The tier.** The body now states HIGH under rule 1 ("CI steps"): `ci/remote/job-common.sh` is the shared gate job of both repositories, and it decides how each gate container runs. 016c's round 1 raised it from STANDARD. This closes that finding.
2. **Each `docker run` and its network** (the body's table, checked against the head's `job-common.sh`):
   - `container` is `run_container none`, and the new `online` is `run_container bridge`. Both keep the earlier flags (`--rm --init`, the label, the limits, the user, `HOME`).
   - `fetch.sh` and `fetch-public.sh` run `online`, the same default bridge as before.
   - The botster-core gate after a fetch step runs `container` with `-e CARGO_NET_OFFLINE=true`: `--network none` as before.
   - The botster-contracts gate (no fetch step) runs `online`: the bridge as before, because it downloads crates and `npm ci`.
   - The volume `chown` and the artifacts copy now run with `--network none`. Neither downloads.
   - Every `docker run` in `ci/remote` goes through these, or carries its own `--network none`. `docker build` is unchanged, and the body says so. No reference to the old `network` array remains. `"$gate"` calls a shell function by name, which bash allows.
3. **No step's command changes.** Only the network of the volume `chown` and the artifacts copy changes, from the bridge to none.
4. **Evidence.**
   - botster-contracts gate: see below.
   - botster-core with this file (`p6/evidence-network-none` `78f0eeaacf67b1c7fa648cd51f4034c76629a7e0` = Core #282 head `017f01277f7e903154b85ba2d9316c97aa50f42a` plus this `job-common.sh`), `~/botster-sessions/gates/botster-core-p6-evidence-network-none-78f0eeaa-pool-20261011-140033-46162.log`, SHA256 `ffc0d0fa802d70d393039d26a00c978e79e0aae6d6201419eab056f6a24253d7`. The fetch steps and the gate container ran: fmt, clippy, taint, public-api, prebuild-worker, slow and fuzz PASS.
   - The three FAIL rows are the byte-copy difference, as the body states: `lists` ("ci/remote/job-common.sh is not the pinned file of botster-contracts"), `test-budget` (only `xtask lists::tests::the_pinned_job_common_is_the_checked_in_copy`), and `mutants` ("FAILED Unmutated baseline", on the same test). Core takes the file as a byte copy at its next pin move.
   - The pool lead's `ip monitor link` record: no veth for the containers that run with no network. The contracts gate still makes one veth pair, as stated, until P6's follow-up adds a contracts fetch step.
5. **Prior art.** Before this change, only the botster-core gate after a fetch step ran with `--network none`.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-p6-job-network-none-16fdb778-pool-20261011-135352-10955.log`, SHA256 `d4f0a2f9e5d759598f36ba0a5f047c46c58a1c61b7a073f43f1ec7a4dd67477d`.
- Header: head `16fdb77854c1fd3859e4645797c927dfa1a8195f`, base `7df311e282260b29fadc143fdd0a97c1645bf1e8` (BOTSTER_CI_BASE_REF), Linux pool. The PR's merge base is the same commit, and it is the main tip, so the head fast-forwards main.
- Result (read by me): all 10 steps PASS; `botster-gate: exit 0 after 441 s on msa1`.
