# Integration review: PR #255

## Round 1: readiness control and v1 import

VERDICT: NOT CLEAN (2 open)

- Initial head: `81304f8588cac4c98deffad2e05ce6fa5fb70b3a`.
- Initial base and sole parent: `517c22350de462256f2b5be0a9fa913a98d0e42f`.
- Replacement head: `b013fdeb77341a142321c256ad57d1ecc078aed2`.
- Replacement base: `5ac94fabf41909a733267a62b692c1a4ee72bf62`.
- Replacement parents: the initial head and replacement base.
- Replacement tree: `2c734a12b81b884a25672d4ede9ee5f287b2346b`.
- READY: `msg_plugin-w_1791713330_b0dd27`.
- Import receipt: `msg_plugin-w_1791713498_b62777`.
- Package NOT CLEAN: `c2d69877b154004e7602fb95e19959ff0b7e3e04`, verdicts/rc-real-controls.md, #255 Round 1.
- Package receipts: `msg_plugin-w_1791713556_ed9025` and `msg_plugin-w_1791713578_fcee9c`.
- Risk: HIGH, shared testkit.

Sol read the complete package round, complete four-file diff, new tests, and surrounding control, row, and wake paths.
Both findings remain on the replacement head.

### PR-SI-1 MEDIUM: the real control accepts a removed session

Status: OPEN. This is package RC-PR-1.

The new real `program_ready` arm reads `session_processes` at real/program.rs:539.
That method reads historical tap rows. Its real.rs documentation explicitly states that a row outlives removal.
`EdgeTap::write_row` records each written row. `delete_row` deletes the inner storage row without deleting the historical record.
The testkit control uses `session_row`, which reads and decodes the current storage row.

After Create and completed Remove, the real control finds the historical row and returns `{ready: false}` for a never-started session.
If that session's probe connected, the historical probe record can return `{ready: true}`.
The testkit returns `Bad`, because no current storage row exists.
The two tiers disagree. The real fence can accept a session that Core removed.

Required correction: read and decode the current stored row for this control.
The existing `RealCoreHarness::stored` method supplies that read through the storage edge.
Keep historical process records for the controls that need them.
Prove `Bad` through `program_ready` after completed Remove on both tiers.
The regression must assert control behavior. It must not assert private historical records.

### PR-SI-2 LOW: the hello test duplicates readiness proof through a helper

Status: OPEN. This is package RC-PR-2.

`the_hello_records_the_probe_and_wakes_the_hosts` directly asserts `Programs::has_connected` before and after the hello.
That helper reads membership in the private probe table.
The new real-session test already proves readiness through the public `program_ready` control.
BUILD.md Testing rule 3 forbids duplicate proof and helper-internal assertions.
Sol raised this point in `msg_plugin-w_1791713493_733457`. The package reviewer agrees.

Required correction: remove those two helper assertions.
Retain the acknowledged protocol request and the observable HostWake signal assertion.
Update the test name, comment, and PR body to describe the remaining wake proof.
Keep the readiness assertions in the tests that call the harness control.

### Other source and merge checks

R-60 defines a read-only readiness control. The testkit returns true immediately for a session with a current row.
The real control reads connection membership without waiting, pumping, releasing a hold, or consuming a report.
The probe registers before `serve` signals each live watched host. The state lock is released before those signals.
The existing Input report wakes remain. The two adapted tests account for the new hello wake.
The room test consumes the hello wake before it waits for the Input wake. Its timeout is a marked deadline.
The real proof uses the guarded production worker and probe. No production crate or product hook changes.
The common argument parser rejects unknown fields. The tapped control list and dispatch include the new name.
The plain harness still rejects controls.

Sol checked the unchanged probe source at contracts-v0.1.29.
It performs terminal setup before its control connection and hello. The .30 probe source is identical.
The .30 tag adds a real-PTY test of that setup boundary. This control-only PR moves no pin.
No pinned .29 transcript or runner uses `program_ready`.
No own dependency, decoder, exclusion, timeout, transcript, pending file, or minimum-list edit exists.
This PR counts no new minimum ID. The later .32 pin PR needs its own reviewed fenced evidence and gate.

The replacement merges #254 through v1 5ac94fab. That base tree equals reviewed and gated #254 head 6ca56167.
Sol's independent `git merge-tree --write-tree` result equals the replacement tree exactly, with no conflict.
All four own added/deleted line sequences equal the initial head's own delta.
The five imported paths contain exactly #254's reviewed changes. The two controls and both test sets remain.
The shared real.rs path requires manual delta review; matching patch identifiers alone do not grant automatic review carry-over.
This manual review finds no additional import issue. It does not close either finding above.

### Prior art

Sol searched OLD CORE at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` for readiness and probe-control mechanisms.
No matching `program_ready` or probe control socket exists there.
The PR reuses #233's program-control dispatch, probe identity map, socket protocol, row access, and host-wake mechanism.
The readiness read adds no new terminal model, dependency, or process implementation.
PR-SI-1 requires the existing current-row reader rather than the historical-row reader.

### Retained evidence and limits

Sol read the initial exact-head Linux log's header, base, suite results, all twelve mutation outcome lines, summary, and completion:
`~/botster-sessions/gates/botster-core-stage1-rc-program-ready-81304f85-pool-20261011-025755-50233.log`.
All ten steps pass, total 465.3 seconds. The default tier passes 1616 tests in 14.148 seconds.
The slow tier passes 466 tests in 32.068 seconds.
Stage 1 catches two of seven mutants and misses five, with zero timeout or unviable mutants.
The unmutated slow baseline passes 466 tests in 37.985 seconds; its complete step takes 54.3 seconds.
Stage 2 catches exactly the five missed full identities once each, with zero missed, timeout, or unviable mutants.
Sol independently compared the complete miss and stage-2 identity sets. They are equal.
The five cover both readiness constants, `serve`, the dispatch result, and deletion of the readiness arm.
The final result is seven caught. The complete mutation step takes 172.8 seconds.
The real pending report retains 58 IDs, with zero newly passing IDs.
The raw minimum still includes unfenced AD-7; R-59 does not accept that unfenced proof.
Fuzz performs no new trial because no changed crate has a decoder harness.
The job and wrapper exit 0 after 553 seconds; the job has one queued second and 552 running seconds.
This gate covers 81304f85, not b013fdeb or any correction.

Sol read the source-identified Mac log's header, relevant results, failed assertion, and summary:
`~/botster-sessions/gates/botster-core-rc-program-ready-81304f85-mac-20261011-025659.log`.
It identifies the full initial head and macOS 27.0.1. The summary reports 410 passes and one failure in 12.337 seconds.
The new public readiness proofs, hello wake proof, and both adapted wake tests pass.
The unchanged zombie-identity test fails: `Absent` instead of `Matches`. This is not an entirely green Mac run.
Sol also read the complete plain-v1 log supplied in `msg_plugin-w_1791713547_8d20ad`:
`~/botster-sessions/gates/botster-core-rc-plain-v1-517c2235-zombie-mac-20261011-031147.log`.
Its header identifies detached 517c2235, zero changed files, macOS 27.0.1, and Darwin 27.0.0.
Its selected test fails with the same assertion and values. The PR does not change that test or its identity implementation.
This retained comparison supports the pre-existing failure classification. It does not erase the failure.
The claimed merged-head Mac result has no named retained log yet; Sol grants no separate merged-head execution claim.

Both findings must close before CLEAN. The corrected final head needs its own full gate against the current v1 tip.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (2 open) at b013fdeb77341a142321c256ad57d1ecc078aed2


## Round 2: current row and public readiness proof

- Head: `15633946e020a9d092ff2fcb8d0bf20560efd202`.
- Previous head and sole parent: `b013fdeb77341a142321c256ad57d1ecc078aed2`.
- Base: `5ac94fabf41909a733267a62b692c1a4ee72bf62`.
- Tree: `c3ff9d9412fa5d96a0289adb9d3ec0fbf6e845f6`.
- Correction receipt: `msg_plugin-w_1791713750_217702`.
- Package CLEAN: `e33b773945b18c50abc6836abd62d1c3a9b7f424`, #255 Round 2 in verdicts/rc-real-controls.md.
- Package receipt: `msg_plugin-w_1791713880_361a32`.

Sol read the complete correction, the complete package round, and the updated GitHub body at this exact head.
The correction changes three files. It adds 66 lines and deletes 27 lines.

PR-SI-1 / RC-PR-1 MEDIUM is CLOSED.
The real control calls `stored`, which reads the current storage edge and decodes the row with Core's decoder.
An absent or invalid row returns `Bad`. The decoded worker identity supplies the probe lookup.
The historical process record no longer determines whether the session exists.
The testkit proof completes Remove with `OpResult::Ok` and checks `Bad` through the control.
The real proof removes a never-started session, drains the host pump, and checks `Bad` through the control.
Its public `session_processes` read establishes that the historical record remains. It reads no private table.
Sol checked the unchanged host removal path: storage deletion precedes release; release schedules completion as the next pump step.
The helper drains all reported pump work and polls events before it checks the absent session.

The real readiness proof now arms the payload hold before Start.
It observes the recorded identity through `registry_row`, then checks `{ready: false}` through `program_ready`.
The hold prevents payload launch even if the launch action is not yet due.
After release, the proof pumps until `{ready: true}`. No helper membership assertion supplies that readiness proof.
This test claims no crash or adoption at a held lifecycle step.

PR-SI-2 / RC-PR-2 LOW is CLOSED.
The hello test removes both direct `has_connected` assertions.
It retains the acknowledged protocol fence and the observable one-signal HostWake assertion.
Its name, comment, and PR body describe the wake proof.

The inherited #254 controls remain intact. The prior manual source-import review remains applicable.
No pin, transcript, pending file, minimum ID, dependency, exclusion, timeout, or production crate changes in this correction.
The prior OLD CORE search and reuse assessment remain applicable.

Sol read the exact-head Mac checks' header, relevant checks, corrected proofs, failure, and summary:
`~/botster-sessions/gates/botster-core-rc-program-ready-15633946-mac-checks-20261011-031421.log`.
The header identifies the full head, zero changed files, and macOS 27.0.1.
Formatting, crate clippy, taint, and lists pass. The three corrected public readiness and wake proofs pass.
The crate reports 410 passes and one failure in 12.202 seconds.
The unchanged zombie assertion still returns `Absent` instead of `Matches`.
The retained plain-v1 comparison from Round 1 reproduces that failure. Neither Mac crate run is entirely green.
The log's ad hoc merge metadata does not prove the plan's four automatic carry-over conditions.
Sol uses the prior manual source review, not that metadata, for the import.

The body reports two reversal checks. No named retained reversal log was supplied; Sol records no independent execution claim for them.
The initial 81304f85 Linux gate does not cover this correction.
A full green gate on this exact head and current base remains required before merge. Sol awaits its completion receipt.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 15633946e020a9d092ff2fcb8d0bf20560efd202
