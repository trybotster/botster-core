# PC transcript audit

This is a preparation audit, not an implementation verdict.
No PC implementation head has arrived for review.

Inputs:

- Core harness base: PR #220, `6c1624e940cd98da24690b1224fc1c5fa8893c6e`.
- Contracts: `contracts-v0.1.24`.
- Plan: revision 23s, `~/botster-sessions/pins/stage1-plan.8a9727e5.md`.
- P3 mapping: `~/botster-sessions/shared/core-stage1/p3-program-controls-real-forms.md`.

The reviewer read all 32 program-control transcripts at the pinned contracts tag.
Step numbers below start at 1.
The reviewer ran no tests and changed no transcript, timeout, or expected value.

The reviewer later verified plan revision 23y at `~/botster-sessions/pins/stage1-plan.65f78ee1.md`.
Core now pins `contracts-v0.1.26` at `v1` base `c19ad938bd87c60331bd002c0117102d8756b1ae`.
All 32 audited transcript blobs are unchanged between `contracts-v0.1.24` and `contracts-v0.1.26`.
This audit remains applicable to those transcripts.

## Kernel-buffer questions

P3 maps `pty_chunk` to the probe's read size.
P3 maps `pty_accept` to the probe's read limit.
Neither control limits the worker's write to the kernel buffer.
The following transcript conditions therefore need a steward decision before their IDs leave the real-pending list.

| ID | Steps | Evidence |
|---|---|---|
| `conf::a2_2_cancel_race_reports_the_real_outcome` | 14–20 | `pty_accept` permits one probe byte. A five-byte write must then complete Partial or Cancelled with both counts equal to 1. |
| `conf::am_2_next_write_waits_for_completion_or_partial_end` | 12–17 | A one-byte probe read precedes cancellation of a five-byte write. The transcript excludes Written. |
| `conf::in_2_link_loss_is_unknown_not_zero` | 4–8 | A one-byte probe read precedes worker loss. The transcript requires Unknown, although the kernel can accept the complete five-byte write first. |
| `conf::in_2_partial_is_certain` | 4–13 | A one-byte probe read precedes cancellation. The transcript requires Partial or Cancelled, with a positive count below 5. |
| `conf::in_6_cancel_reports_progress` | 4–11 | A one-byte probe read precedes cancellation. The transcript excludes Written and equates the probe log length with the write count. |
| `conf::in_7_lost_worker_ends_pending` | 4–9 | A one-byte probe read precedes worker loss. Both small writes must remain pending and complete Unknown or NotWritten(SessionEnded). |

These are setup questions. The audit does not claim a measured failure.
The audit proposes no replacement expectation.
The steward must decide how the real tier establishes the required partial or unacknowledged write.

The lead confirmed the review rule in message `msg_plugin-w_1791629017_cb8e14`.
All six IDs must stay in `core-real-pending.txt` until the steward rules.
The reviewer must flag any PR that removes those IDs before that ruling.
The lead sent these questions and the read-fence gaps below to the steward.
The lead says the seven minimum IDs are not affected and go first.

### Steward ruling relayed by the lead

Message `msg_plugin-w_1791629051_a11546` supplies the following requirements:

1. Establish held state through option (1). The probe stops reading through `pty_blocked`. The harness writes until a write is Partial or would block. The transcript's small write follows that state.
2. The steward rejects testkit-proven status because a real form exists under A5-4.
3. Fix over-assertions by ID and step, only where the assertion fixes a kernel number.
4. The lead's exact outcome wording is: "IN-2 Partial or Complete; IN-6 Cancelled with progress; IN-2 link loss Unknown; ordered complete pty_input log once reads resume".
5. The lead says: "\"Excludes Written\" stays where the clause excludes it while the write is held."
6. After the seven minimum IDs are done, extend this audit with one line per ID, step, assertion, and clause.
7. Send the extended audit to the lead. The steward must rule on each line before a transcript changes.
8. Keep all six IDs in `core-real-pending.txt` until that work is complete.

This document preserves the lead's outcome wording. It does not define a new code identifier or replacement expectation.

## Observation timing

P3 defines `pty_input` as the bytes that the probe read.
IN-2 counts bytes that the worker wrote to the PTY.
Completion of a worker write does not establish completion of a probe read.

The following positive log assertions have no preceding `await_pty` fence for their full expected length:

- `conf::a2_2_cancel_race_reports_the_real_outcome`, step 9: one byte after Written.
- `conf::am_2_next_write_waits_for_completion_or_partial_end`, step 8: six bytes after two completions.
- `conf::in_5_lane_bounds_refuse_at_begin`, steps 19 and 20: four bytes and three bytes after completions.

The implementation review must check these conditions against the control's observation boundary.
The reviewer must not strengthen `edges_quiet` to mean that a worker or probe finished.
Plan section 8 defines quiet as no arrived or unread edge input.

The lead assigns these read-fence gaps to contracts transcript fixes under the R-43 process.
The fixes use `await_control` on the `pty_input` log with the expected content, not one poll.
The fixes may proceed with the assertion audit or earlier as a separate change.

## Terminal translations

Only two `pty_output` requests in this group contain a literal newline byte:

- `conf::ad_3_adopted_equals_started`, step 5: `68690a`.
- `conf::dp_3_screen_is_one_frame_within_max_screen_frame_bytes`, step 4: 1,966 bytes, including CR/LF pairs.

Neither transcript asserts equality between those output bytes and route output bytes.
The first transcript compares terminal state and screen reads before and after adoption.
The second transcript accepts any screen payload.
This audit found no raw-byte equality conflict caused by those two newline requests.

The literal input payloads in this group contain none of the input bytes that P3 lists as translated, consumed, or signal-producing.
Those bytes are `0x0d`, `0x03`, `0x1a`, `0x1c`, `0x13`, and `0x11`.
The key inputs in this group use `a` or `left_shift`.
Expected key bytes remain the libghostty oracle's responsibility.

## Minimum IDs

The six kernel-buffer questions concern non-minimum IDs.
The seven minimum IDs can proceed through the agreed probe controls and P6's route dependencies.
This statement does not establish that any minimum ID passes.
Each ID still needs its real-tier proof on the implementation head.
