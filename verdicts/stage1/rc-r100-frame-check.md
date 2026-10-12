# Core PR #309: integration review

## Round 1

- Reviewer: Sol-C.
- Branch: `stage1/rc-r100-frame-check`.
- Head: `84be34b0b8bc464bf6be55af2ef2c292b670fcdf`.
- Source base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- Reviewed delta: the complete nine-file diff and the receiving worker, link decoder, admission, and send contexts.
- Scope: R-100 frame checks, shared launch construction, frame sizing, refusal effects, and the R-91 queue-size interface.

### RC-R100-I1 — HIGH — Launch admission exceeds the receiving worker's initial bound

`frame_fit.rs:178` checks the Launch against `HostEngine::link_frame_bound`. That bound now uses six times the largest configured payload plus 1 MiB. However, `Worker::new` creates its decoder with `DEFAULT_MAX_PAYLOAD`, which is 1 MiB. `Worker::on_launch` applies `LaunchSpec.link_frame_bound` only after the decoder accepts Launch.

A request can therefore pass Create and Start but fail on the link. For example, with default limits, an ASCII argv field larger than 1 MiB fits the derived bound. The host admits the call and encodes the Launch. The worker rejects its frame length before it can read the new bound, then closes the link. This violates R-100 across the host/worker boundary.

The new host tests use scripted worker reports. They do not prove that the actual initial worker decoder accepts an admitted Launch.

Required change: make Launch admission use the bound that the receiving worker applies before Launch. Alternatively, establish an agreed bound before Launch under the protocol rules. Add a regression from host admission through the shared worker decoding boundary. Check the exact bound and a message one byte above it.

Owner receipt `msg_plugin-w_1791784909_607bb0` confirms the defect. The owner will use `DEFAULT_MAX_PAYLOAD` for Create, Start, and Created-session notification-policy Launch checks. The owner states that changing the initial protocol-1 bound would conflict with R-98. The fix and regression remain due on a replacement head.

### Existing package findings

The package reviewer owns these findings. This integration round retains their open status and adds no duplicate finding.

- **RC-R100-1 — HIGH — omitted modifier vectors.** The fast path omits `KeyInput.mods` and `MouseInput.mods`. Those fields are vectors without a length bound. A large valid modifier vector can pass payload sizing and the fixed-part assumption, then exceed the frame bound. The package reviewer requires complete sizing or contract-valid validation, plus a public admission/link regression.
- **RC-R100-2 — MEDIUM — private-state assertions.** The Create and attach tests inspect private session and route storage. The package reviewer requires public results or boundary observations, including proof that attach admits no route or event.
- **RC-R100-3 — HIGH — pending setters can enlarge Launch.** Start checks the current request. A previously admitted Created setter can replace the color profile before SendLaunch. A valid palette can enlarge a near-bound Launch after Start passes. SizePolicy can also enlarge the encoding. The package reviewer requires sizing for all values that can change before Launch and a public queued-setter/Start regression.
- **RC-R100-4 — LOW — gate base wording.** The body names `ae49bbec` as the gate base. The completed gate header names `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`. The body must distinguish the source base from the gate-time reference.

Package finding receipts: `msg_plugin-w_1791784839_13a006` and `msg_plugin-w_1791784899_9ead3c`. No exact-head CLEAN package verdict is available.

I subsequently read the published #309 package round at `10182ac69f63800fc109c9ba4883ae102448a5b9`, file `verdicts/rc-real-controls.md`. It covers the same head and retains these four package findings plus RC-R100-I1. Both reviews stop this head's remaining audit while the owner prepares the repair.

### Other integration checks

The send path and check use `launch_spec_for`. The frame check precedes operation allocation and commit. The attach check precedes route allocation and handoff registration. `PayloadTooLarge` retains its existing precedence. The link counter measures the same serde JSON representation as the encoder.

The six-times factor covers a text byte that JSON encodes as a six-byte escape. The cap remains `u32::MAX / 2`. In #307's reviewed queue design, `largest_frame` uses the frame bound plus the header, and the queue formula uses saturating arithmetic. Thus the larger frame bound increases that queue term. This round accepts no future #307 composition; that composition needs a delta review.

The PR changes no conformance list, contracts pin, dependency, or mutation exclusion. The Git whitespace check reports no error. The dropped key-bound cache is absent from this diff.

### Gate evidence and disposition

I read the retained gate log:

`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-r100-frame-check-84be34b0-pool-20261011-225141-51725.log`

The header names the reviewed head. All ten steps pass. Mutation totals are 66 mutants, 60 caught, six unviable, zero missed, and zero timeout. The job and wrapper exit zero. These results do not close the source findings above.

The implementer requested a hold of this head's review while preparing one replacement commit and gate. This round records the current findings. I ran no build, test, gate, or pool job. A replacement head needs a delta review and exact-head package verdict. Applicable train reviews and the final train gate remain required.

VERDICT: NOT CLEAN (5 open)
