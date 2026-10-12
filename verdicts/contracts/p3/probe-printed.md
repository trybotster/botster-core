# contracts #80 — p3/probe-printed (R-62 follow-up: the probe's own prints), reviewer 017f

HIGH (rule 3): the PR changes three packages (`botster-probe-script`, `botster-conformance-probe`, `botster-core-conformance`). The probe is the program edge of every real-tier Core transcript. Package reviewer: 016c.

## Round 1: NOT CLEAN on 18ff91b92f76dd50c8f8bb77e1f5058b455720ad (1 HIGH)

The head merges main `41355cc6603bf9a384bcfe6082b7cf21e855b87d`. Main is now `bf50290e08222bd47386ae6e02ab997a6a0b1826`. `git diff 41355cc6 bf50290e` changes only `docs/steward-rulings.md`, so this is a rulings-only move: no new merge, no new gate.

Checked, no finding:
- **The protocol** (`botster-probe-script/src/control.rs`): `PRINTS_ENV`, `Report::Printed{bytes, after}` and `Request::Recorded{}` meet condition 1 of the ruling. The report goes before the write, and the probe waits for the harness's answer.
- **`control::Order`**: while a print waits for `Recorded`, an `Output` request is deferred, not queued and not acknowledged. On `Recorded`, the print is queued first, then each deferred `Output` with its `Ack`. So the order of the reports on the socket is the order of the writes, and `after` counts the `Output` requests queued before the print. `Wire` keeps the socket writes and the queue behind one lock. `Wire::print` waits on the condvar, which releases the lock, so the serve thread can take `Recorded`.
- **a2_1** (Q2): only the two `print` items change. Each `0a` becomes `1b44` (10 per print, 20 bytes in all). No expectation changes.
- **The lint** (condition 3): a print with `0a` in the program of a session that an `oracle_resume` reads is flagged once, at the `Create`. The self-test covers two resumes, `0d1b44`, and another session.
- **The controls doc**: condition 1 names the reports, the harness's two checks (the script's bytes, `after`), and keeps `fork_child` refused. Condition 4 covers the print bytes.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 267 s on msa1, base main `41355cc6`; mutants 20 caught, 7 unviable, 0 timeout: `~/botster-sessions/gates/botster-contracts-p3-probe-printed-18ff91b9-pool-20261011-191106-58751.log`.

### F1 (HIGH): in control mode a print is now only queued, so a later `exit` or `signal_self` can drop it; the change applies with or without `PRINTS_ENV`

- Before this PR, the main thread wrote each print to stdout itself (`write_all` and `flush`), and only then ran the next step.
- Now `print()` (`main.rs`) calls `Wire::print`. `Order::print` gives `Effect::Queue(bytes)`, and `Wire::print` returns once the bytes are in the writer thread's channel, not once they are written. With reports off (`PRINTS_ENV` unset), it returns at once.
- The next step can then be `Step::Exit`, which calls `std::process::exit` (`main.rs:141`), or `signal_self`. Neither waits for the writer thread (`main.rs:192-197`). So the queued print can be lost, completely or in part.
- On the real tier the probe always runs in control mode: the harness sets the control socket in the `env` of each `Create` (`docs/core-testkit-controls.md`, "The real-tier forms of the program controls"). Ten transcripts have a print directly followed by `exit`: `a2_1_read_screen_history_flag_and_text`, `a2_2_write_to_ended_is_ok_not_written_session_ended`, `a2_3_every_route_close_reason_maps_to_one_wire_reason`, `ou_2b_every_core_close_reason_maps_to_one_wire_reason`, `ou_2b_healthy_reasons_send_route_closed_last`, the four `ou_7_*` exit ids, and `st_5_reads_after_exit`. Their prints are what the routes and reads assert.
- So the PR body's claim "Without `PRINTS_ENV` … a harness at `contracts-v0.1.41` (Core v1 today) is unchanged" is not true, and neither is "the pin alone changes nothing". The `.44` pin would make these ids flaky on the real tier: a false failure, not a false pass, but on ids that gate every Core PR.
- No test catches it: the slow tests that print and then exit (`the_probe_prints_scripted_bytes_and_exits_with_the_scripted_code` and others) run without the control socket, where `print()` still writes directly.

Fix: a print step returns only when its bytes are written, as before. For example, the writer thread acknowledges each queued print (a reply channel), and `Wire::print` waits for that acknowledgement after `Recorded`, outside the lock. That keeps one writer and the reported order, and keeps the old rule that the next step runs after the print's write. Waiting at `exit` for the whole queue is not the same: a `pty_output` write that the terminal blocks (OU-3d) would then hold the exit, which the probe never did. Add a slow test in control mode, with and without `PRINTS_ENV`: `print` then `exit`, and the terminal reads every printed byte before the exit status. It must fail on this head. Correct the two claims in the body.

### Open items (not findings)

- **The serving Core PR** is not open yet. The ruling's owner line asks this PR to name it. The body gives the lead's order (`.44` tag, RC's pin PR, then the Core PR) and says the number is added when it opens. I accept that order. After F1's fix, the pin is again opt-in.
- **P3's question to the steward**, whether condition 4 and the lint cover every O_raw read (`oracle_screen`, `oracle_modes`, `oracle_state`), is open. It does not change this head. If the steward widens the rule, I re-check the lint delta.
