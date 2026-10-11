# Contracts PR #46 — R-66 route-close fence

## Round 1 — source review, 2026-10-11

Repository: `trybotster/botster-contracts`.
Head: `9d19b997fd044ca76c0844f8a32fa79d930e5ec4`.
Base: `5a54f7af6c8f309717a39b11c7c175c1335c289b`.
Parents: `a823aa9971773c9e340ff649b5fdb30132ac5359` and the base.
Tree: `ad7dcb8cd3074aace5f3fb3cacd3e1bddd84c672`.
Risk: HIGH. The lead assigns this integration review and relays its verdict to the steward.

I read the complete three-file own delta, the full affected transcript, vocabulary entries, control row, and live body.
I read the driver's await_control implementation and the retirement and EV-5 clauses.
I read the complete steward correction. Its verified SHA256 is `f351b0866c3771333fdba49b816c836271347d62008e62dbac2a5d65ee47ab26`.

The transcript replaces the post-Detach quiet wait with await_control on the consumed close report.
The driver pumps without polling or a clock step. It checks the control before the first pump and after each pump.
The next control read requires activity_before_close and closed. Thus the later retirement assertion requires a consumed activity report.
The change retains the earlier quiet wait and every subsequent expectation and barrier.
The vocabulary accepts the required session and route arguments. The transcript declares the new required control.
The control row separates consumption from posting. It states both deferred cases and all three post conditions.
This ev_5 case uses Detached, a Running session, no parked route work, and the default pump_events budget.
The accepted Core #271 source supplies the same consumed-at-link-edge meaning on both tiers.
This review accepts no general event-post fence or engine signal.

### Base import

I read the complete a823aa99-to-9d19b997 delta. It contains only the base's approved R-67 paragraph from #43.
The import edits the same documentation file, so this review does not call it a disjoint import.
Independent comparisons confirm the complete own diff matches the earlier own diff after index-line removal.
The transcript and vocabulary blobs stay unchanged. The merged document keeps the R-66 row and the base's R-67 paragraph.
No new source finding appears.

### Finding and package artifact

**CR-R66-1 LOW — premature plan citation. CLOSED.**
The initial body attributes the steward correction to plan 23at, although the lead says that plan update remains pending.
The owner replaces that citation with the correction's exact path, verified hash, and items 1 through 4.
The body states that the plan correction remains pending. I read every body change at the unchanged new head and base.
The remaining plan citations describe the established control-before-pin order and combined tag.

I read the crate reviewer's complete local ACCEPT for this exact head.
Artifact: `/Users/jasonconigliari/botster-sessions/trybotster-botster-contracts-stage0-review-crate-catchup/.review-verdicts/r66-route-reports-9d19b997.md`.
SHA256: `90146fe62c1140957c306d434c8ae6686df8d8b00cde7368ab3b4b354b8cfed8`.
The artifact reports zero open findings. It also reports that automatic approval review blocked its GitHub publication.
The stated rejection reason is missing explicit authorization for external disclosure. This review does not claim that artifact is published.

### Historical evidence and limits

I read the Mac transcript records: five failures before the change, ten real passes after it, and five testkit passes.
The failures name seed zero, step 28. The records name Core `82e20c3a2a7d5f3b6e55510643b2e5ac57c37c76` and Darwin 27.0.0.
The changed records use local contracts `ef33e94fe360b5160d6583db8238fd87ba36c8b2` and local pin/list edits.
I read the supplied script without executing it. I use the explicit individual test results, not its pipeline exit as proof.
Static reads from the supplied scratch repository confirm its transcript and vocabulary equal the current reviewed source.
The later documentation correction does not alter those executed inputs.
I independently compare all 690 identifiers and outcomes in the full before/after testkit records. Their multisets match.
Each record reports 209 passed, zero failed, and 481 ignored. Execution order differs.
These historical records keep their original heads and local-edit scope. They do not approve a Core pending removal.
Linux proof of the changed transcript remains with the later combined pin gate under R-66.

I read the a823aa99 gate header and final summaries. All ten steps report PASS in 427.9 seconds.
The mutation step selects no mutant. The wrapper exits zero after 494 seconds.
That earlier gate does not count as execution at 9d19b997.
The completed exact-head gate and the package artifact's publication remain pending before merge acceptance.

This verdict completes source review with zero open findings. The lead owns the merge decision and steward relay.
I ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open)

### Exact-head gate receipt — 2026-10-11

The owner supplies the completed gate at unchanged head `9d19b997fd044ca76c0844f8a32fa79d930e5ec4`.
The base remains `5a54f7af6c8f309717a39b11c7c175c1335c289b`.
Log: `~/botster-sessions/gates/botster-contracts-p3-ev5-route-closed-fence-9d19b997-pool-20261011-080851-83269.log`.
SHA256: `7ec1a6ace2dfdbf8f6cc6b9462fcb654e434d3a077d33a4f8367deb86c433bd1`.
I read its exact head/base header and final step summaries. All ten steps pass in 183.0 seconds.
The mutation step selects no mutant. Jobq exits zero after 192 seconds; botster-gate exits zero after 193 seconds.
This completes the exact-head gate receipt. The source verdict remains CLEAN with zero open findings.
The package's local ACCEPT is read and hash-verified. Its external publication remains unconfirmed.
The lead owns the publication disposition, steward relay, and merge decision. I ran no execution job.

### Publication disposition — 2026-10-11

Lead receipt: `msg_plugin-w_1791731758_bbc33b`.
The lead reports that the steward merged #46 using the crate reviewer's local verdict file and verified hash.
The steward requires no GitHub review. Thus external publication is not a remaining acceptance condition for this PR.
This disposition supersedes the earlier statement that package publication remains required before merge acceptance.
The permission refusal remains in force. The lead will not relay earlier user authorization, and 016c will not retry the post.
No publication action or approval request remains. The source CLEAN and exact-head gate receipt remain accepted.
The lead supplied no merge SHA. I make no independent merge-tree claim in this receipt.
No Core pin move or pending removal follows. I ran no execution job.
