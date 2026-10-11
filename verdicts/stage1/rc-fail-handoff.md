# Integration review: PR #246

VERDICT: CLEAN (source review; final exact-head gate remains required)

- Head: `b168c192ff58459b4671834bf01b87cc823c6fb7`.
- Base: `8dab35d14f7038d3cc0598e2cde8da207bd1b723`.
- Head tree: `f1961d3f7fe9561bdddc9dbbb80dd955b6cf3fd2`.
- Package CLEAN: `ce391f1d3a8c4f6e33a0e2a2054419bb773c3c7b`.
- Branch: `stage1/rc-fail-handoff`.
- Risk tier: HIGH.

## Source and contract

Sol read the complete four-file diff against the base.
Sol also read the changed methods, the host driver, the real edge, and the existing testkit control.
The control matches the `fail_handoff` row at contracts-v0.1.29.
That row requires one failed endpoint handoff, a `RouteClosed` event, and the end of the client's stream.
The unchanged DP-2 transcript checks both public observations.

Each tap starts with a false flag.
The control accepts no arguments and arms only the named handle's tap.
The existing handle lookup rejects a closed or unknown handle.
The plain harness reports the control as unsupported.
The control list, dispatch, and both control-list proofs include the new control.

The hold check runs before the failure check.
A blocked handoff retains the endpoint and the armed flag.
The next eligible handoff consumes the flag and returns the same endpoint with `DescriptorSendError::Failed`.
The tap sends no byte and gives no descriptor to the inner edge.
Later handoffs reach the unchanged inner edge.

The existing host driver drops the returned endpoint and removes the complete handoff frame.
It queues `Input::HandoffFailed` for that route.
The real edge already returns this error form when a descriptor send fails.
This PR uses that boundary. It changes no production driver, process owner, cleanup path, or worker protocol.

The new edge proof observes returned endpoint ownership and the fake edge's recorded calls and bytes.
Those records are observations of the edge boundary. The proof reads no private Core queue.
The PR adds no wait, timer, process spawn, transcript edit, or timeout change.

The pending list removes only `conf::dp_2_failed_handoff_closes_route_handoff_failed`.
`conf::ou_2b_handoff_failure_is_reported_as_handoff_failed` remains pending.
Its later removal needs proof after the combined route implementation in #243.
This review does not approve that later removal.

Package finding RC-FH-1 is closed.
The complete delta from `a8d9a156` moves the hold comment directly above `hold_on`.
It changes one documentation line's position and changes no executable code or test.
GitHub identifies `b168c192` as the published head.

## Prior art

Sol checked OLD CORE at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The descriptor-send searches found no `SCM_RIGHTS`, `sendmsg`, or `recvmsg` implementation there.
Its routed-envelope code supplies no equivalent endpoint-handoff fault control.
There is no old descriptor-handoff implementation to reuse for this control.

The new code reuses this repository's tap, argument parser, handle lookup, and host-edge error type.
The existing testkit control fails the same handoff boundary once.
The real tier needs a small tap flag because its inner edge owns real streams.
No dependency or separate state model is needed.

## Focused Mac evidence

Sol read the complete retained record:
`~/botster-sessions/gates/botster-core-rc-fail-handoff-merged-mac-20261011-015021.log`.

The header identifies `11db5ae43bd59ce8d0fa1eceae5cf1f646a391b0` plus the DP-2 pending-list edit, with contracts-v0.1.29.
Sol checked that the complete delta to `9fb4a854` is that pending-list edit.
The later delta to `a8d9a156` changes the fake initializer and inherited gate code only.
The final delta to `b168c192` moves the documentation line only.
The real control and real process path therefore retain the tested source.

DP-2 passes on both tiers: testkit 1/1 in 0.02 seconds, real Mac 1/1 in 4.69 seconds.
The second-route transcript passes on the testkit tier.
It remains inconclusive on the real tier at step 9, with `step_timeout`, and stays pending.
This record supplies focused Mac proof for DP-2. It supplies no full Mac-suite claim.

## Gate evidence and the approved re-gate

The first exact-head gate at `a8d9a156` failed:
`~/botster-sessions/gates/botster-core-stage1-rc-fail-handoff-a8d9a156-pool-20261011-020457-95993.log`.
The slow step passed. The unmutated stage-2 baseline failed only on EV-5.
The error is seed 0, step 21, `PendingLimit`, with `pending_ops is 4`.
The baseline reports 428 passed and 1 failed. Fuzz did not run.
The wrapper exits 1. This record is not a green gate.

Package cause confirmation `19849546` states that EV-5 never arms this control.
Sol checked that the false flag forwards the same descriptor call and changes no completion, admission, or slot-release decision.
This PR leaves the missing worker descriptor intake and the unfenced recovery poll unchanged.
The package cause confirmation gives no source CLEAN by itself.

The lead records written approval for one re-gate in the live lead handoff at the entry labelled approximately 11:15 PT.
The entry names this head, the package confirmation, the plain-v1 trace comparison, and the removing fenced pin.
Sol accepts the same specific mechanism classification used for #249 under approved plan 23al.
The plain-v1 Mac samples showed `UnknownRoute`; this gate showed `PendingLimit`.
The retained classification uses the same recovery step, pre-poll state, source diagnosis, and trace evidence.
It does not assert reproduction of the exact `PendingLimit` error on plain v1.
Shared pre-poll counts alone would not establish the cause.
EV-5, R-63, and A21 remain separate work.

Sol read the completed approved re-gate:
`~/botster-sessions/gates/botster-core-stage1-rc-fail-handoff-a8d9a156-pool-20261011-021055-12442.log`.
Its header identifies the full `a8d9a156` head and the full `8dab35d1` base.
All ten steps pass. The CI total is 228.3 seconds.
The default tier passes 1615 tests in 12.093 seconds.
The slow tier passes 429 tests in 28.413 seconds, including real DP-2.
The real pending report retains 76 IDs and counts none of them as passed.
The wrapper exits 0 after 238 seconds. The job runs for 237 seconds after one queued second.

Stage 1 tests nine mutants. It catches three and misses six.
The caught mutations replace `Tap::fail_next_handoff` with `()` and the tap's descriptor-send result with `Ok(0)` or `Ok(1)`.
The unmutated stage-2 baseline passes all 429 tests.
Stage 2 catches exactly the six stage-1 misses:

- `RealCoreHarness::fail_handoff` returns `Ok(Default::default())`.
- `RealCoreHarness::has_control` returns `true` or `false`.
- `RealCoreHarness::control` returns `Ok(Default::default())`.
- The `fail_handoff` dispatch guard becomes `true` or `false`.

Sol compared the full identities in both stages.
The final result is nine caught, zero missed, zero timeouts, and zero unviable mutants.
Fuzz reports no changed crate with a decoder harness. It performs no new fuzz trial.

Sol read every configured exclusion and its reason at the reviewed head.
The complete exclusion file equals the base file: 206 regular expressions and two test globs.
No configured expression excludes the changed tap or real-harness implementation.
The test globs exclude test files only.
The gate checks 238 cited names, the gate decisions, and 207 exclusions against 6000 listed workspace mutants.
The extra exclusion is the inherited off-Mac expression.
Listing exclusions is a static check. It is not mutation proof by itself.
The inherited prebuilt-worker limit remains unchanged.

## Acceptance limit

The green gate ran on `a8d9a156`, not `b168c192`.
Source CLEAN and focused Mac evidence carry across the reviewed documentation delta.
They do not replace the required full gate on the final head before merge.
Any later v1 import also needs its base-merge record and a full exact-head gate.
The known #233 conflicts need review of the final control-list and dispatch union.
Sol ran no build, test, gate, mutation job, reversal, or measurement job.
