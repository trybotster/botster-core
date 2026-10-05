# PR #141 — P6 control registry integration review

## Round 1 — Module registration and existing dispatch

Reviewed head: `7e9b4ec6e7252c3ff7ec36cbff74c24fdae644fe`.
Reviewed base and merge base: current v1 `01fd38968b9e7605becc7e2b5088628aff52865a`.
The lead explicitly assigns this one-crate PR to integration review.
The lead limits this review to existing control behavior, module registration, and RefusalScript rules.
The P6 reviewer retains the remaining package scope.

The complete delta changes controls.rs, harness.rs, lib.rs, and refusal.rs in botster-core-testkit.
The registry stores function pointers with the harness, handle, and argument value as inputs.
Each module can register its own controls through ControlRegistry::register.
The collector calls the module registration functions. Future modules need no change to harness.rs dispatch.
Duplicate names fail during construction. Lookup uses the same registry for discovery and dispatch.
The existing registration contains only fail_next. Unknown names still return ControlError::Unsupported.

The reviewer compared the moved fail_next parser with the implementation on current v1.
The target and error fields, default occurrence of one, integer conversion, and error text remain unchanged.
The returned Null, typed Refused payload, and Bad errors remain unchanged.
refusal_script returns a clone of the same Arc-backed RefusalHandle stored for the named handle.
The handler therefore arms the same script that RefusalLayer consumes.
RefusalScript::arm and its sync-column validation, occurrence, conflict, and typed-result rules remain unchanged.
The move does not script asynchronous failures or add a production test branch.

The reviewer read the focused Linux log:
`~/botster-sessions/gates/botster-core-stage1-p6-control-registry-7e9b4ec6-linux-20261004-192647-83597.log`.
It records 198 unit tests and one doc test passing, Clippy with slow for all targets, and formatting.
The existing fail_next tests and new registry tests pass. The job exits zero after 181 seconds.
This focused log supplies no full landing-gate acceptance.
No dependency pin, machine, process edge, native terminal behavior, or conformance pending id changes.
The PR preserves merged M1. It adds no control capability beyond fail_next.

No source finding is open in the assigned integration scope.
The exact-head package verdict remains required before integration CLEAN.
The implementer must run the required landing gate after both reviews.
The reviewer ran no tests, builds, or gates and left .gitignore unstaged.

VERDICT: NOT CLEAN (1 open acceptance item; exact-head package verdict required)

## Round 2 — Exact-head package acceptance

Reviewed head: `7e9b4ec6e7252c3ff7ec36cbff74c24fdae644fe`.
The reviewed delta remains the four testkit files recorded in round 1.
Current base and merge base remain `01fd38968b9e7605becc7e2b5088628aff52865a`.
The reviewer read package CLEAN verdict `db280708bbdf9ecdef5df04d0dbef2bf07c6c3aa`, verdicts/p6-control-registry.md.
That verdict covers this exact head and has zero open findings, including LOW findings.
The pending package acceptance item from round 1 is CLOSED.

All three checks assigned by the lead pass at the source-review level.
Existing control names, arguments, results, and errors remain unchanged.
Modules can register their controls without changing harness.rs dispatch.
The refusal registration keeps the shared script and all RefusalScript rules.
The PR preserves merged M1 and changes no conformance pending id or production path.
Round 1 records the supplied focused evidence and its limits.

This CLEAN accepts PR #141 within the lead's assigned integration scope on this exact head.
The implementer must run the required full gate before merge.
A later head requires delta review.
The reviewer ran no tests, builds, or gates and left .gitignore unstaged.

VERDICT: CLEAN (0 open)
