# contracts #77 — stage1/pc-r88-held-write (R-88: the R-74 held-write fence for in_9, tm_6_begin and in_6_refused_full), reviewer 017f

HIGH: transcript changes to `in_9_modes_read_at_write_start` (live on both Core tiers) and `tm_6_begin_and_attach_signal_wake`, and the list in `held_write.rs` that the gate reads. Package reviewer: 016c.

## Round 1: NOT CLEAN on 13bfac1bde292e3d5fea78b999a575d853c81396 (1 LOW; main merge and gate still to come)

One commit on main `13ad6720480647e32ab30c63c0f7125a2afd6918`. The ruling: R-88 on main (`13ad672`, the `in_9` item) and R-74's held-write form (F bytes, `pty_blocked {kernel_bytes: 1}`, then `await_control pty_queued` with `bytes ≥ 1`).

Checked, no finding:
- **in_9.** s3 `pty_blocked {on, kernel_bytes: 1}`, s4 `let $fill = repeat 61 × 40960` (`HELD_WRITE_FILL_BYTES`, `lib.rs:45`), s5 the fill write `$w0`, s6 `await_control pty_queued` with `bytes $between [1, 40960]`. Then the focus write `$a` (s7) queues behind `$w0`, so its write starts only after s10 unblocks the PTY, which is after s9's `ModesChanged`. So s11's `Completed{$a}` with `pty_bytes_written > 0` reads the modes at write start (IN-9), and the race in PC's record (step 4's write not held, `Completed{$a}` before `ModesChanged`) is gone. s11's `pump_until` passes over `Completed{$w0}`. `requires.controls` adds `pty_queued`.
- **tm_6_begin.** The 1-byte write at s18 becomes the R-74 form (s17 `kernel_bytes: 1`, s18 the fill, s19 the fill write, s20 `pty_queued`). The pump at s23 (`more: false, events_posted: 0`) and the cancel at s24 (`Admitted`) then meet a write that is surely pending.
- **in_6_refused_full.** `pty_chunk 1` and the 2-byte write become the R-74 form. The `pty_queued` fence at s10 comes after `SessionWritable` and before the `Admitted` cancel, so the write is surely still pending at s11.
- **`held_write.rs`** adds all three ids to the list of R-74-form transcripts. The R-74 form test checks `kernel_bytes` and `pty_queued` in each.
- **`fake-replacement-map.md`:** in_9 and in_6 leave FakeCore's behavior table, and the counts move from 167/474 to 165/476. FakeCore stays frozen and has no `pty_queued` (R-74), so these ids become `fake: structural`.

### F1 (LOW): the `fake: structural` citation is R-88, but the ruling that makes held-write transcripts structural is R-74

R-74 (`docs/steward-rulings.md:313`): "FakeCore stays frozen … The held-write transcripts are `structural` on FakeCore". The four existing held-write ids cite `"ruling": "R-74"` and README `R-74`. #77 gives `in_9_modes_read_at_write_start` and `in_6_refused_full_wakes_on_room` `"ruling": "R-88"` and README rows `R-88`. R-88 chooses the fence. R-74 is why the fake cannot run it. Please cite R-74 in both transcripts and both README rows. `tm_6_begin` keeps `R-12`, which was its reason before, and is not affected.

### Still needed before the final CLEAN

- **The main merge.** Main is now `6fdfc413f5bc1a32ca2cea6c39cbb47780f1bd4f` (#75, #69 and rulings). No path is shared with #77, and `git merge-tree` has no conflict. But #75 changed `fence_lint.rs`, which lints these three transcripts, so this is not a rulings-only move. Merge main once, add the Q7 script output, and run ONE gate on that head. The READY names no gate yet.
- **The Linux real-tier job** that the READY announces, with its log in the body.
- **#73 overlap.** #73 (CLEAN on `69301401`) has R-88 `debt:` lines owned by PC for `tm_6_begin` (steps 20-23) and `in_6_refused_full` (step 9). This PR fences them. Whichever of #73 and #77 merges second removes them (Q7 delta review).
- `in_6_refused_full` stays UNSUPPORTED_CONTROL (`control_queue`) on both Core tiers, as before. It is pre-existing and owned by PC, so there is no run evidence for its new form.

## Round 2: no open finding on c284e82c51aaaed2e1d14f66a1510c3931a6bf97 (final CLEAN when the Linux per-seed log is in the body)

- **F1: closed.** `d9cb11be` changes only `"ruling"` to `R-74` in in_9 and in_6_refused_full, and their README rows to `R-74`.
- **Merge of main `6fdfc413` (Q7): disjoint, no finding.** The tree `cf1acb6862cd46f3b38e9499bd55c3da1550e4d9` equals `git merge-tree d9cb11be 6fdfc413`. `git patch-id --stable` of the PR's own diff is `3420b363255c39afc72a38e24f1817cf8742e3a7` against both bases. PC's script and log: `shared/core-stage1/evidence/pc-r88-held-write/q7-merge-main-6fdfc413.log`.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 410 s on msa1, base main `6fdfc413` (current main): `~/botster-sessions/gates/botster-contracts-stage1-pc-r88-held-write-c284e82c-pool-20261011-175745-13986.log`.
- **Still to come:** the Linux per-seed Core job (in_9 and tm_6_begin, seeds 0-7, and the in_9 red-on-revert) with `c284e82c`'s transcripts, which the READY announced. in_9 is live on both Core tiers, so I give the final CLEAN when its log is in the PR body.

## Final: CLEAN on c284e82c51aaaed2e1d14f66a1510c3931a6bf97

- Linux per-seed log: `~/botster-sessions/shared/core-stage1/evidence/pc-r88-held-write/r88-linux-c284e82c.log` (msa1, Linux 6.12.111, Core `69d41499`). The job's sha256 of each of the three transcripts (`resolve-linux-job.out`) equals `c284e82c`'s blob (`d6f2f0c6…`, `fd1612b6…`, `2c830106…`; I computed them from git).
  - in_9: seeds 0-7, each alone, 8 of 8 PASS on the real tier. tm_6_begin: 8 of 8 PASS.
  - Red on revert, with the scratch delay: this PR's in_9 PASS; the `.41` in_9 is INCONCLUSIVE at `seed=0 step=8 limit=step_timeout` (log lines 216-218).
  - Run 1 failed at its clone (no network) before any test, so it is not a run.
- Gate PASS on the exact head (round 2), base main `6fdfc413`, which is still current main.
- Every finding is closed. #73 (final CLEAN on `fa1b7620`) still has PC-owned debt lines for `tm_6_begin` (steps 20-23) and `in_6_refused_full` (step 9). Whichever of #73 and #77 merges second removes them, and I review that delta.
