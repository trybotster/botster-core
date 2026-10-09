# Contracts pin review

## PR #161 P5 Round 1 — 2026-10-09

- Exact head: `f81578858b137528aca82496904e89d16d6262ea`.
- PR branch: `stage1/contracts-v0.1.14`; P5 work branch: `stage1/p5-pin-v0.1.18`.
- Accepted v1 base: `c869dbeaf3a2922e4f4e7202f8e55490f7ce99fa`.
- Tree: `f5912d9af570edc253291949084457cb46587abd`.
- Previous reviewed head: `dea90ed4ee502432a70e19ec93b26c4a81558773`.
- Previous CLEAN verdict commit: `d1a3869e68f90c0ab69a951141304776e0818eb8` on stage1/review-p6.
- Contracts tag: `contracts-v0.1.18`, commit `ae4abc9bf71a2a80f3609d91a0c3c71c5b863efc`.

The prior review history remains in `d1a3869e:verdicts/p6-contracts-v0.1.14.md`.
The lead assigned #161 to P5 and retargeted the pin to v0.1.18 in the shared lead handoff.
The reviewer read the prior verdict, complete merge delta, pin delta, changed contract API, relevant amendments, current PR body, and supplied gate.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

### Merge and pin

Merge `0fe5ba7ad6330bf3fa067fc23948e47c8e420fc1` has the previous reviewed head and accepted v1 head as its parents.
The reviewer compared each retained edit against its corresponding base.
All seven files retain the prior pin edits, including the lockfile sources, ledger, pending IDs, and three R-30 citations.
The merge adds no other change relative to accepted v1.

All six workspace contracts dependencies use v0.1.18.
All nine contracts lockfile entries use the exact tagged commit.
After normalization of those source entries, the lockfile is identical to the merged v0.1.17 lockfile.
No package version, checksum, or dependency list changes.
The new tag contains R-35 and R-36 and the Core A13, A15, and A16 crate changes.

### Contract API changes

Observation::ClipboardWrite now carries `contents: Option<Vec<ClipboardContent>>` with the contract's own type.
The serde field options match the contract event's options. The link adds no second type or byte encoding.
The host passes contents directly into Event::ClipboardWrite and preserves selection, total_bytes, and reason.
This path preserves a missing value, an empty list, and the order and bytes of each MIME representation.
The adapted host proof checks the MIME name and bytes after the observation travels through the link.
The queue proof retains ClipboardWrite as a droppable event.

No worker source emits this observation yet. The PR body assigns its later binding-to-wire mapping to P3's M2b work.
This pin review does not prove the complete A13 or A14 clipboard behavior.

The new CoreLimits.max_key_text_bytes defaults to 256 and validates the range 1 to 4,096 in the contract crate.
Core's open check calls that validator. The only CoreLimits struct literal in Core's source uses the default for remaining fields.
The A15 host admission check remains separate work, as the PR body and pending list state.

OpOutput::ServiceEnd replaces OpOutput::ServiceExit in the contract crate.
Core has no StopService completion that needs this adaptation yet. A16 remains pending under P7.
The route codec, probe script, and test-support source remain unchanged between the two contracts tags.
The pin delta adds no dependency, mutation exclusion, platform-specific code, or native binding change.

### Ledger and review scope

The checked-in Core ledger equals the new tag's sorted Core IDs, byte for byte: 675 IDs.
The new tag adds no Core ID relative to v0.1.17.
The pending IDs remain identical to the previous reviewed head.
Only the two A15 owner comments change in the new pin commit.
They now name P1 lifecycle, worked by P5, as stage1/plan owners.py requires.
The deferred and withdrawn copies match the new tag, byte for byte. The Core deferred table remains unchanged.
The PR body has the required Prior art note and states the previous review and new API scope.

### Supplied exact-head gate

The full Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/pin-v0.1.18-f8157885.log`.
It names the exact reviewed head and accepted v1 base.
The default tier reports 920 tests passed and 675 skipped. The slow tier reports 243 tests passed and 962 skipped.
The in-diff mutation run reports two caught, zero missed, zero timeouts, and zero unviable.
All ten listed CI stages pass. Both link decoder fuzz runs complete without a crash. The job and gate exit zero.
This Linux evidence does not establish execution of Mac-only code.
The earlier red gate remains historical evidence; this verdict uses the new exact-head gate.

No finding remains open in this pin and merge delta.
This verdict closes no conformance ID, deferred behavior, or separate package proof hold.
Integration owns its cross-package review. The lead owns the merge decision.

VERDICT: CLEAN
