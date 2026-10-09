# Guardian proof role review

## PR #180 Round 1 — 2026-10-09

- Exact head: `1fe19991c61c3665f4695ca0c63cbbca09b1f6c6`.
- Branch: `stage1/p5-guardian-proof-role`.
- Base: `1832866c0f7787a442dc29745aca27c41162bc2e` on v1.
- Tree: `63e8d5a0b5d5a1fb419d316cbf721222d5f06cbb`.

The reviewer read the complete two-file delta, link proof rules, guardian caller, lifecycle proof, current PR body, and supplied gate.
The reviewer checked the AD-6 and DP-8 clauses, P7 brief, and current plan.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

### Source and proof

Link::authenticate now checks host_proof for the received hello.
Link::hello retains token_proof for the guardian's sent hello.
The protocol, instance, and epoch checks remain present. An invalid hello changes neither the proved epoch nor the authenticated state.
The shared proof module binds the sender role into the hash. The guardian now checks the host role, as the worker does.
No other crate depends on botster-guardian-core at this head. The package name, wire fields, and proof rule remain unchanged.

The lifecycle test builds host and guardian hellos with their respective proof roles.
All four sent-hello assertions use the guardian role.
The new reflected case supplies the correct token, instance, protocol, and epoch with the guardian role.
The test requires LinkClose, no launch action after refusal, and the original orphan deadline.
The existing higher-epoch acceptance and lower-epoch refusal checks remain present.
The reviewer found no source defect in this delta.

### Supplied evidence

The full Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/guardian-proof-1fe19991.log`.
It names the exact reviewed head and base.
The default tier reports 920 tests passed and 654 skipped. The slow tier reports 243 tests passed and 941 skipped.
The in-diff mutation run reports six caught, zero missed, zero timeouts, and zero unviable.
All ten listed CI stages pass. Both guardian decoder fuzz runs complete without a crash. The job and gate exit zero.
The delta adds no dependency or mutation exclusion and changes no platform-specific code.
This Linux gate does not establish execution of Mac-only code.

### GPR-F1 — LOW — The PR description has no Prior art note

Status: OPEN. The reviewer sent this finding directly to P5.

The inspected PR body has What, History, Tests, and Evidence sections, but no Prior art note.
The binding pair-common.md requires a Prior art note for every PR and requires the reviewer to reject a PR without it.

Required change: add the note to the PR body.
Name the reused host_proof and token_proof functions in botster-core-link.
State the rejected alternative and its reason, if an alternative was considered.
Explain any new hand-rolled piece, or state that the delta adds none.
A body-only correction needs no new head or gate.

This verdict covers the guardian proof-role delta only. It closes no conformance id or separate P5 adoption proof hold.

VERDICT: NOT CLEAN


## PR #180 Round 2 — 2026-10-09

- Exact head: `1fe19991c61c3665f4695ca0c63cbbca09b1f6c6`.
- Branch: `stage1/p5-guardian-proof-role`.
- Base: `1832866c0f7787a442dc29745aca27c41162bc2e` on v1.
- Tree: `63e8d5a0b5d5a1fb419d316cbf721222d5f06cbb`.

### GPR-F1 — CLOSED — The PR body has the required Prior art note

The reviewer read the corrected PR body and verified the unchanged exact head and base.
The note names the existing host_proof and token_proof functions from botster-core-link.
The note explains that the existing two roles cover the guardian, so P5 rejected no alternative.
The note states that the delta adds no hand-rolled piece and preserves the fixed proof rule.

Round 1's source review and exact-head Linux evidence remain valid.
No review finding remains open in the guardian proof-role delta.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
This verdict closes no conformance id or separate P5 adoption proof hold.

VERDICT: CLEAN
