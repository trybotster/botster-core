# DP-5 input-log cost integration review

## Round 1: source and completed initial gate

Exact head: `0572493fc9e695acd57b1916e485fdd900ba7643`.
Tree: `c5f49dde29d14b45d5079cfd4a6c1df66cfea0d6`.
Source and gate base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Lead assignment: `msg_plugin-w_1791836886_ad222e`. The lead requires #324 before #329.
Sol-B reads the complete five-file patch, full body, caller contexts, and complete package section at `f0a5525aefc42ee0ae38232fc7d63833c53cd280`.
The source diff check passes.

Controls retains its byte log and adds an initially empty hex string.
Its private take_input method appends the same bytes to both representations under the existing mutex.
Static search finds no other production input_log mutation.
Both existing append sites call take_input: an accepted write prefix and queued input delivered when blocking ends.
The blocked queue remains outside both logs until release.
Partial acceptance, input limits, errors, script delivery, and queue order retain their previous behavior.

The session pty_input and testkit route pty_input return the cached text.
The real route still reads its byte log and encodes it once per response.
Both route callers pass lowercase hex to the private route_pty_log helper.
That helper retains the same bytes object and unattributed segment. An empty log has no segment.
The pinned .44 codec's HexBytes::to_hex calls the same hex_encode function.
That encoder emits two independent lowercase digits per byte, so concatenated encodings equal the whole-log encoding.
No control shape, field, transcript, or product behavior changes.

The regression compares cached text with the original encoding after direct input, partial acceptance, blocked queueing, and release.
It checks the empty log and bytes 0x00, 0x0f, 0xf0, and 0xff.
Existing session, route, and kernel-queue proofs remain applicable and pass.
The cache retains two characters per logged byte. Responses still clone the cached string.
This removes repeated encoding; it does not remove every copy or establish a universal timing gain.

The held record changes owner to GS, timing descriptions, and its removal trigger to plan 23cn.
The identifier remains budget-held and pending. #324 implements no held-full accounting change.
The local profile and timing comparisons remain owner reports.
The retained local log names e37b91d6 plus a working tree, not the committed candidate.
Its cleanup statement remains an owner report; its local timing and strict results supply no pool timing claim.
The body includes the prior-art note and states HIGH.

Completed gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-dp5-fill-cost-0572493f-pool-20261012-115236-30950.log`.
SHA256: `7a166e03a4ec1d89136a355fa4fafdfa2e6adc1ae2601088f30befe572f84b3a`.
The header names this exact head and base. All ten steps PASS, with a 509.4-second total.
The job and wrapper exit 0 after 1178 seconds on msa1: 588 queued and 590 running.
Default passes 1958 tests, with 427 skipped, in 12.517 seconds.
Ordinary slow passes 633 tests, with 2059 skipped, in 43.863 seconds.
The held-full run covers three budget identifiers at 32 seeds, one trial per seed.

Mutation stage 1 catches eight of nine mutants and misses the real SessionInput::log replacement.
The unmutated stage 2 baseline passes 633 tests, with 2835 skipped, in 43.187 seconds.
Stage 2 catches that replacement with conf::am_2_one_admission_point_orders_all_sources.
Final results are nine caught and zero missed, timeout, or unviable.
Sol-B independently matches all nine raw identities to citations and verifies every catcher in the ordinary PASS results.
No runtime FLAKY or INCONCLUSIVE result appears. The inherited EV-2 exclusion earns no catch credit.
The fuzz step selects no changed decoder harness and passes without a harness run.

No source, interface, body, or supplied-evidence finding remains. The package and integration results agree.
No budget, timeout, seed count, dependency pin, gate check, or mutation exclusion changes in this patch.
This CLEAN accepts the source and completed initial exact-head gate.
The base has advanced. Current-base delivery, assigned train review, and lead merge checks remain due.
This receipt accepts no later head, merge, #329 accounting result, or additional retry.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; exact PR head and completed initial evidence)
