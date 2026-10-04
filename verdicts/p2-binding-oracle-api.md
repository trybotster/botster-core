# P2 binding API review for P6

VERDICT: CLEAN

Reviewed head: `a088d673d759984d9b10823d17d713b45c006ce9`.
Branch: `stage1/p2-binding-oracle-api`.
Review base: `393f403047beb1583ab3a711afbe2c37255c7e64`.
Open findings: 0, including LOW findings.

## Scope and findings

The review covers the public binding APIs that the lead authorized as a prerequisite for the P6 terminal controls.
It does not approve the P6 controls or their harness dispatch.

- Each native call exists at fork `3f8eb6810bb673aa782b047de21783ac81fb1121`. The fork pin and fork source remain unchanged.
- The C declarations match the pinned types, style layout, data keys, and render keys.
- `from_snapshot` uses the native transactional decoder. It enables continuation retention and sets image storage to zero before restoration.
- The wrapper frees the decoder after success or failure. A constructed `Terminal` owns its handle and callback buffer on later failure.
- Unsupported envelope versions produce a typed error after the native decoder refuses the snapshot. Other decoder errors retain their library classification.
- Hyperlink, cell, color, cursor, continuation, failure, and image reads obtain their values from native APIs.
- Cursor inspection documents its effect on render dirty state. Decoder documentation states the required source geometry and history configuration.
- The production APIs have no test branch or test-only feature. Test modules contain the allocator fixture and assertions.
- New terminal byte literals are stimuli. Expected terminal state and bytes come from libghostty reads or restoration comparisons.
- The audit records each API, its clause, its native source, its proof, the retention limits, and the lead's graphics ruling.
- The prior-art note identifies the reused private decoder path. No old repository code or old tests were copied.
- The added mutation exclusion names only `Render::drop`. Its reason describes the unobserved native cleanup call.
- No real-process test or protected testkit file changes in this prerequisite.

## Supplied verification

The reviewer ran no tests, mutation job, or gate.

The implementer's `/private/tmp/p2-oracle-final-focused.log` identifies the exact reviewed head.
It reports 126 tests passed, clippy with `-D warnings` passed, and formatting passed on Linux.

The implementer's `/private/tmp/p2-oracle-mutants4.log` identifies head `69f4c915cd63dea30f08f40a0bf844e324b929c1` and the same review base.
It reports 43 mutants tested: 31 caught, 12 unviable, zero missed, and zero timeouts.
The delta from that head to the reviewed head changes only tests and audit documentation.
The production code and mutation exclusions are identical across those heads.

The full gate remains the implementer's next step on the exact CLEAN head.
Any later commit requires review of its delta.

The reviewer kept `verdicts/p6-testkit.md` unchanged and left the pre-existing `.gitignore` edit unstaged.
