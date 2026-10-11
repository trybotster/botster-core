# Integration review: PR #259

## Round 1: contracts-v0.1.32

- Head: `6b20f6f2116f34526cf6ebf09e263776fff35123`.
- Base: `d86bfcc747718452efe57b2aa207492acde96573`.
- Sole parent: `ef4eee4a633152639934d68722296fd05a02f115`, whose sole parent is the base.
- Tree: `dd13b585e9f8031d7ea965326e627ae6b6e68323`.
- READY: `msg_plugin-w_1791715483_4ec84c`.
- Package CLEAN: `c2c1cf3cf33fe9d281568cc949d1981dc066de09`, verdicts/rc-real-controls.md, #259 round 1.
- Package receipt: `msg_plugin-w_1791715644_1c3149`.
- Risk: HIGH. The contracts pin changes.

Sol read the complete four-file Core delta and complete package round.
Sol checked the contracts interfaces, normalized transcript delta, tag identity, pending lists, and retained evidence.
No finding remains open. The delta adds 26 lines and deletes 23 lines.

All seven workspace dependencies use contracts-v0.1.32. All nine contracts lock entries use its peeled commit:
`91394e726b1bd697307d0d5b05712b876f9189be`.
The lock changes only source references. Versions and dependency lists remain.
The five pending reasons and one status-test comment name the new tag.
The named A15 cases and A17/A18/A19 groups still have no transcript at this tag.
Both Core pending files retain exactly the same IDs: 464 core-pending and 58 real-pending.

The approved plan puts this pin after program_ready and start_held. Both controls exist on the reviewed base.
The contracts driver maps UnsupportedFor to unsupported_control with the missing input's name, never a pass.
Its await_control returns success only when the matcher accepts the answer. A timeout or poll limit is inconclusive.
The start_held fences therefore require held:true before the held lifecycle step is acted on.
The R-60 fences require program_ready before the relevant input. The compression byte changes replace only 0a with 2e.
Sol independently checks every changed compression string: lengths stay equal, and every other byte stays equal.
R-62 adds title fences and the specified IND replacements. Its three real oracle_resume trials remain pending.
This pin supplies no comparator implementation or real oracle_resume proof. R-62's independent-oracle conditions remain.
No frozen contract, ledger, deferred file, withdrawn file, or BUILD.md changes between the two tags.
No Core machine, process owner, test assertion, production hook, minimum list, exclusion, or timeout changes.
The mutation configuration is byte-identical to the base's previously reviewed configuration.

The PR has a Prior art note. Sol checked OLD CORE at 72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c.
Its Cargo.toml has no contracts or conformance dependency. This pin uses the existing workspace dependency mechanism.

### Evidence

Mac testkit logs:
`~/botster-sessions/gates/botster-core-rc-pin32-compare-testkit-mac-20261011-033646-v1-v0.1.29.log` and
`~/botster-sessions/gates/botster-core-rc-pin32-compare-testkit-mac-20261011-033646-pin-v0.1.32.log`.
Their headers name the reviewed base and head, with clean worktrees and macOS 27.0.1.
Each normal and strict run has 690 unique IDs: 206 pass, 484 are ignored, and zero fail.
Sol independently compares the complete result multisets, including pending annotations. Both pairs are equal.
Normal times are 0.53 and 0.57 seconds. Strict times are 1.47 seconds on each source.

Mac real log:
`~/botster-sessions/gates/botster-core-rc-pin32-real-mac-6b20f6f2-20261011-033646.log`.
Its first header identifies the exact head and .32 pin. Its second line retains a stale .29 label.
It names six local real-pending removals for these trials. Those IDs remain in the committed pending file.
The fenced ad_7_payload trial passes in 1.74 seconds.
The three route cases are inconclusive at steps 5, 5, and 4. The three ST-6b cases report unsupported oracle_resume.
Sol counts none of those six trials as a pass. The Linux gate independently proves the committed fenced AD-7 form.

Full Linux gate:
`~/botster-sessions/gates/botster-core-stage1-rc-pin-v0132-6b20f6f2-pool-20261011-033824-56127.log`.
The header identifies the exact reviewed head and base. All ten steps pass in 286.7 seconds.
The default tier passes 1624 tests in 13.260 seconds. The slow tier passes 467 tests in 31.845 seconds.
The fenced ad_7_payload real trial passes in 0.168 seconds. Its transcript's step 4 requires held:true.
R-59's fence condition is met for this ID on the reviewed head. This verdict accepts no other held lifecycle model.
Real-pending remains 58, with zero newly passing entries. The gate reports real-passing 40/68 and real-accepted 41/69.
The diff has no mutant, so no mutation run starts. Fuzz starts no trial because no changed crate has a decoder harness.
The job and wrapper exit 0 after 354 seconds; the job queues zero seconds and runs 354 seconds.

Plan section 8 Q8 accepts this one green full gate when both CLEANs cover the same exact head and base.
No repeat after CLEAN is required. Sol sent that correction to the package reviewer.
A later source head or base import needs the applicable review and its own exact-head gate.
This verdict establishes no merge. Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 6b20f6f2116f34526cf6ebf09e263776fff35123
