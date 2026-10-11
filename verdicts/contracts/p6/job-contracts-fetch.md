# Contracts #65: integration review (p6/job-contracts-fetch)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
PR: https://github.com/trybotster/botster-contracts/pull/65 (P6, `sess-1791518250-0169-fa157d48c599262bb69c87d0b928c13e`). Package reviewer: 016c. Tier: HIGH (BUILD.md risk tiers rule 1: CI steps), stated in the body. The follow-up that #63 states.

## Round 1: head `71f01c8812132fa979b8584f12a92ba2efde071e`

VERDICT: CLEAN (0 open) on this exact head. Merge-ready from the integration review, by fast-forward.

### What I checked and found correct

1. **The contracts fetch step.** The new `ci/remote/fetch-public.sh` runs `cargo fetch --locked` into the cargo volume, and `npm ci` of `ts/` into the npm cache, when `npm` exists (a Mac node has none, and the gate's `ts` step skips there too). It runs `online`, with no token.
2. **The job.** `fetch-public.sh` now runs whenever the file exists, not only after the token fetch, and it sets `gate=container`. After any fetch step, the gate runs with `--network none`, `CARGO_NET_OFFLINE=true` and `npm_config_offline=true`. A project with no fetch step still runs its gate `online`.
3. **botster-core does not change in behavior.** Its order is the same: the token fetch, then `fetch-public.sh`, then the gate with no network. The only addition, `npm_config_offline=true`, has no effect: Core v1 (`origin/v1`) has no `npm` or `npx` in `xtask`, `ci` or `.config`.
4. **Evidence.**
   - The gate ran the `ts` step (`npm ci`, build, typecheck, tests) in the gate container with no network, and it passed. So the fetch step fetches every download of the contracts gate.
   - The pool lead's veth record (job `jobq-botster-contracts-71f01c88-20261011140835-6376`): only the `fetch-public` container made a veth, for about 2 s. The volume `chown` and the 8-minute gate container made none.
5. **Prior art.** Core's split of a token fetch, a public fetch, then an offline gate.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-p6-job-contracts-fetch-71f01c88-pool-20261011-140835-82726.log`, SHA256 `9e4387c57b18ea8dfd3ce7d4716e72444dfc3fd4c6e345c988b91b49482e53c8`.
- Header: head `71f01c8812132fa979b8584f12a92ba2efde071e`, base `7df311e282260b29fadc143fdd0a97c1645bf1e8` (BOTSTER_CI_BASE_REF, the main tip when the gate started), Linux pool. The log has the fetch step's line.
- Result (read by me): all 10 steps PASS; `botster-gate: exit 0 after 495 s on msa1`.
- The PR's base is main `16fdb77854c1fd3859e4645797c927dfa1a8195f` (#63), which is the head's parent. The gate ran on the exact head, which contains that main tip, and its base diff from `7df311e2` is a superset of the PR's own diff. So this gate is the merging gate, and the head fast-forwards main.
