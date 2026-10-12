# contracts #79 — stage1/pc-unheld-blocked-input (R-74 follow-up: the unheld blocked-input lint and three fences; R-93: the cross-source order lint), reviewer 017f

HIGH: transcript changes to `in_8_mode_checked_at_write_start_by_worker` (live on v1, fails now), `in_9_key_encoded_by_worker_modes_at_write` and `dp_5_every_not_written_reason_maps_to_a_refusal_or_no_frame`, two new fence-lint rules, and 36 new lines in `core-fence-exceptions.txt`. Package reviewer: 016c.

## Round 1: no open finding on d9d4ccfc362ceb1701aa0588944b8bcf0bcc679c (final CLEAN when the Linux per-seed log is in the body)

### Merges (Q7)

- `4be50b2`, `686dace` and `f62a7a4`: each tree equals `git merge-tree --write-tree` of its parents. No conflict.
- `b6256f8` (main `c78ee0b1`): the tree differs from the merge-tree tree `7ef77e43` only at the two conflicts. Both keep #79's side, and both are correct:
  - in_9_key_encoded: s14 `let $floor add [40960, $n]`, then s15 `await_pty $floor`. The fill comes first, so a floor of `$n` alone is reached by the fill (R-56). One `await_pty` remains.
  - `core-fence-exceptions.txt`: main's `in_9_key_encoded step 4 no program_ready, R-60` goes, because s3 adds `program_ready`. No in_9_key exception line remains.
- **Main move after the gate base.** The gate base is `41355cc6603bf9a384bcfe6082b7cf21e855b87d`. Main is now `48e518f1a724189d9c873c2d3b6470a7594c7414`. `git diff 41355cc6 48e518f1` changes only `docs/steward-rulings.md` (R-95, and the R-94 follow-up). This is a rulings-only move: no new merge and no new gate.

### Transcripts

- **in_8.** The fill write, then `pty_queued` (s6), then the paste (s7) behind it, then `ModesChanged`, then `pty_blocked` off, then `Completed` with `pty_bytes_written > 5`. The paste starts only after the mode change, so the race in PC's record (the worker starts the paste with bracketed paste off and completes it `NotWritten(ModePreconditionFailed)`) is gone. `fake: structural`, ruling R-74, as for the other held-write ids.
- **dp_5_every_not_written.** s54-s57 hold s2 with the fill. Route op 1 (s58) queues behind the fill, so `Stop` gives `session_ended` with `written_bytes` absent.
- **in_9_key_encoded.** `program_ready` comes before the fill (R-60). The key waits for `ModesChanged`. The capture floor is the fill plus the key's bytes (above).
- **am_2_fairness_bound_across_routes.** `9bba9eb6` reverts `33e3f306`. The transcript equals main's (no diff). Its R-60 line `step 16` comes back. The text-twin relaxation in `held_write.rs` goes too, because only am_2 needed it. The fill check is again "one hex byte".
- **`held_write.rs`** lists 12 ids. The list has in_8, in_9_key_encoded and dp_5_every_not_written, and not am_2.
- **Evidence transcripts.** The blobs at `cccca161` (the evidence run) and at this head are equal: in_8 `35a7910a…`, in_9_key_encoded `c0d35f31…`, dp_5_every_not_written `85ce4ad6…`.

### Lint rules (integration level; the rule logic is 016c's package scope)

- **`fence_lint/held.rs` (R-74 follow-up).** A window opens at a `pty_blocked` on with no `kernel_bytes`. The rule flags the first Core-state assertion after the window's first input that is a pump or follows one. Probe reads (`pty_input`, `route_expect_pty`, `await_pty`, `pty_queued`, `program_ready`) are not Core-state assertions. This matches the ruling and PC's sweep.
- **`fence_lint/order.rs` (R-93 and its follow-up).** The read kinds and the three fence pairs match the R-93 follow-up text: `pty_queued` after a host input with no host input after it; `route_frames_taken` of a route before the first host input; `route_frames_taken` of each route before off.
- **The exception lines.** 25 `unheld blocked input, R-74` lines and 11 `cross-source order after a hold, R-93` lines.
  - The R-74 debt lines name P3 (e4_1/ev_8) and P5 (st_7). The exception lines (a2_2, dp_5_lane_full s16, dp_5_route_op_namespace, ev_8_fallback_started s27, in_4) each give a reason that agrees with the SAFE list in PC's sweep record.
  - `3cddd830` moves two ev_8 R-74 debt lines from step 48 to step 44, after #76's inserted steps. The window is the same (input at step 41).
  - The 11 R-93 debts are am_2 s22 (owner PC) and the 10 e4_1/ev_8 windows of PC's sweep (owner P3). The two dp_5 windows of the sweep have no line, because their reads list every interleaving.

### Gate

PASS on the exact head, all ten steps, exit 0 after 321 s on msa1, base main `41355cc6` (see the main move above): `~/botster-sessions/gates/botster-contracts-stage1-pc-unheld-blocked-input-d9d4ccfc-pool-20261011-190625-42038.log`.

### Still needed before the final CLEAN

- **A Linux per-seed Core log for in_8 and in_9_key_encoded.** in_8 is live on v1 and fails now. The evidence log `unheld-evidence-20b2525c.log` is from macOS only (`Darwin 27.0.0`): in_8 8 of 8 PASS on the real tier, and red on revert at `seed=0 step=8 limit=step_timeout`. The real-tier race depends on the kernel's PTY buffer, and #77 gave a Linux log for the same held-write form. Please run the same job on Linux (seeds 0-7 for both ids, and the in_8 red on revert), with the job's sha256 of each transcript, and put the log in the body.

### Note for the steward and P3 (not a finding of #79)

The R-94 follow-up (main `48e518f1`, after this head) rules that 3 parked-reply windows admit the host write `62` after the fence and check the bytes after the fill in either order: `e4_1_client_reply_reserves_its_frame_size` s72, `e4_1_reply_one_byte_over_the_free_bound_is_parked` s43 and `ev_8_admission_reserves_full_bytes` s42.
- #79's R-93 debt lines for these windows state the fix as "no admitted host write after the fence". The ruling now admits one.
- `order.rs` counts a host input after the `pty_queued` fence as unordered, and counts any `hex_prefix` check of a bound `pty_input` as an order assertion. So P3's ruled form gets an R-93 finding.
- P3's PR therefore needs either an `exception:` line with the ruled reason, or a lint change. The steward rules the check form (R-94 follow-up), so the steward also decides this. #79 is correct against the rulings at its head.

No open finding. Final CLEAN on `d9d4ccfc362ceb1701aa0588944b8bcf0bcc679c` when the Linux per-seed log is in the body and its transcript sha256 values equal this head's blobs. If main moves by more than rulings before the merge, I re-check the delta per Q7.
