# P6 control registry review

VERDICT: CLEAN

Reviewed head: `7e9b4ec6e7252c3ff7ec36cbff74c24fdae644fe`.
Branch: `stage1/p6-control-registry`.
PR: https://github.com/trybotster/botster-core/pull/141
Review base: `01fd389` (P3 M1 merge).
Open findings: 0, including LOW findings.

## Scope and behavior

The lead authorized this separate refactor after P3 M1 merged.
The lead confirmed that the earlier protected-file restriction ended with that merge.
The integration reviewer also reviews this change because it touches P3's wiring.

- The registered control set remains exactly `fail_next`.
- Discovery and dispatch use the same registry. Unknown controls still return `ControlError::Unsupported`.
- `refusal.rs` owns the registration, argument parsing, and `fail_next` handler.
- The handler preserves the previous parsing, occurrence default, error mapping, and synchronous-column validation.
- `refusal_script` clones the existing shared handle. The handler arms the same script that the opened Core's refusal layer consumes.
- Module registrations use the standard `BTreeMap` and function pointers. Duplicate names fail during harness construction.
- P3's Core construction, worker wiring, scheduler, directory state, and refusal wrapping remain unchanged.
- No production crate, control capability, transcript, terminal expectation, or pending id changes.
- The delta adds no mutation exclusion or process test.

Existing harness tests continue to check discovery, unknown controls, bad arguments, script arming, and typed refusal results.
New registry tests check handler arguments and duplicate registration.
The PR includes the required prior-art note and the reason for using a standard-library dispatch table.

## Supplied verification

The reviewer ran no tests, mutation job, or gate.
The implementer's focused Linux log identifies the exact reviewed head:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-control-registry-7e9b4ec6-linux-20261004-192647-83597.log`.
It reports 198 unit tests and the documentation test passed, clippy with all targets and `slow` passed, and formatting passed.

The full gate and integration review remain required before merge.
Any later head requires review of its delta.
The reviewer kept `verdicts/p6-oracle.md` and `verdicts/p6-testkit.md` unchanged.
The reviewer left the pre-existing `.gitignore` edit unstaged.
