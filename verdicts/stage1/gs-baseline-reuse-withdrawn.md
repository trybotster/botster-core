# Integration record: PR #253 withdrawal

Status: WITHDRAWN. No integration CLEAN verdict exists.

- Head: `6105272829458d691761af3b0ec1b380f80b9413`.
- Reported base: `8dab35d14f7038d3cc0598e2cde8da207bd1b723`.
- Risk tier: HIGH.
- Withdrawal message: `msg_plugin-w_1791710749_959cb5`.
- Published record: https://github.com/trybotster/botster-core/pull/253#issuecomment-6107568683.

Sol received READY, read the PR body, and read the complete delta outside the new record module.
The package reviewer then reported GS-L2-R1-1 HIGH: content hashes omit execution-relevant file mode.
GS asked Sol to hold the review and then reported withdrawal with the lead's agreement.
Sol confirmed that GitHub shows CLOSED at the same head and read the complete closing comment.

The closing comment states four reasons:

- The two runs use different nextest profiles and occur before and after stage 1.
- The slow tests can fail on a second unmutated run. #246's first gate is a retained example.
- A file can retain its content hash after it loses execute permission.
- The measured saving is 31.5 seconds and applies only when stage 1 misses a mutant.

Sol independently read #246's cited gate during its integration review.
Its slow step passes, but its unmutated stage-2 baseline fails EV-5 with step-21 `PendingLimit`.
The record's proposed reuse does not supply that second unmutated result.
This evidence supports preserving that acceptance limit.

Sol did not complete the new module's source review or inspect #253's supplied green gate and comparison job.
The reported package metadata finding is not an independently executed proof by Sol.
The PR's supplied evidence therefore receives no integration acceptance in this record.
The withdrawal includes both proposed shell exclusions. Neither exclusion receives approval here.

The closing comment assigns GS a separate design for stage-2 catches that a flaky test could cause.
That future design requires its own review. This record approves no design or gate change.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.
