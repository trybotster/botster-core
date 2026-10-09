# Integration review: #174 (P5 adoption part 1, host side, branch stage1/p5-adopt-1)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — NOT CLEAN on head 6a09f29b

Reviewed head: `6a09f29b6873084c77c4c2a2462485c894608bfc`. Base: v1 `a0f78fe4c4e4faff1fee926e072ceb5e94ba8649`. The
branch is linear on it, so there is no merge. Scope: the cross-package parts (`botster-core-link`, `botster-worker-core`,
`botster-core-testkit`, the two `botster-worker` helper lines), the contract and steward-ruling alignment, and the
design carries. The host crate's internals are the P5 package reviewer's.

### Checked, no finding

- **The proof roles (`proof.rs`).** `token_proof` is the worker's role (`b'w'`), and `host_proof` is the host's
  (`b'h'`). The role byte comes after the domain, between zero separators. `the_proof_follows_the_documented_layout`
  computes the hash from the documented layout, so a change of the rule fails it (design D4). Both handshakes use the two
  roles: the worker checks `host_proof` (`worker.rs:357`), and the host checks `token_proof`.
- **The two `botster-worker` helper lines** are the approved ones (`session.rs` answers with `host_proof`, and
  `driver_edges.rs:429`). No other line in those files changed. The full gate's slow tier runs both helpers against the
  new worker (29 and 29 passed).
- **`WorkerMsg::Adopted`.** It has the five payload states of R-35, with a round-trip test for each one. The report
  carries the features, the terminal state with the revisions (ST-1, IN-10) and the snapshot formats.
- **`Lost(Other)`, carried from design round 4.** `git grep` at this head: no Core code constructs it. `End` is a total
  crate type, so the fallback arms at `flows.rs:357` and `admit.rs:780` are gone. P1's placeholder at `run.rs:692` is
  replaced by the per-row adoption. The one remaining arm reads a recorded `Lost(Other)` and posts
  `Lost(RegistryCorrupt)` (R-35 correction `c3ed727`). The PR body lists every site with its fix.
- **The one-`Launch` carry** has a default-tier test, `a_worker_accepts_one_launch_in_its_life`. See W1 for a gap.
- **The adoption table** in the PR body matches R-35 (a) to (d). The extra defensive rows (`Running` with
  `LaunchFailed`, `Exited`, or `Running`) use the same reasoning as (d).
- **`.cargo/mutants.toml`** is unchanged. The full gate at this head is green, and the slow-profile in-diff run (code
  equal to this head) has 0 missed and 0 TIMEOUT.
- **`connect_worker` defaults to `None`.** Until the worker binds its endpoint, an adoption of a live worker ends
  `Lost(WorkerUnreachable)`. That is AD-2's indeterminate outcome, so nothing is invented.

### A1 MEDIUM — `Adopt(id)` answers `Unsupported`, which A2-1 does not allow; the stop path drops the intent

`admit.rs:288-293`: `Adopt(id)` of a `Lost(WorkerUnreachable)` session with no kept intent (`row_state` is `None`)
returns `ErrorCode::Unsupported`. This answers the PR's open question. It is not acceptable:
- A2-1's operation table (amendment 2) gives `Adopt` exactly two synchronous codes, `UnknownSession` and `WrongState`. It
  makes `Adopt` legal in `Lost(WorkerUnreachable)` and `Lost(WorkerVersion)`. `Unsupported` is not in that row. (A5-3:
  only a code in the call's sync column may be returned synchronously.)
- AD-2: "`WorkerUnreachable` and `WorkerVersion` may leave a live worker, so ... `begin(Adopt(id))` may be retried".

The no-intent case is reachable today. The stop path's broken link (`run.rs:295-298`) ends the session with
`End::Lost(LostReason::WorkerUnreachable)` and leaves `row_state` at `None`. So `recorded_state()` is the shown state, and
the row **records** `Lost(WorkerUnreachable)`. After that, every `Adopt(id)`, on this handle and on every later handle,
is `Unsupported`. The session can never be retried, although the worker may be alive.

The intent on that path is known: the host was stopping the session. Fix:
1. Every Core path that ends a session `Lost(WorkerUnreachable)` or `Lost(WorkerVersion)` keeps the intent in
   `row_state`. On the stop path that is `Stopping`. Then the row records `Stopping`, and a retry applies the `Stopping`
   rows of the table: the stop is sent again, or `Exited{HostStop}`.
2. Then a row never records `Lost(WorkerUnreachable)` or `Lost(WorkerVersion)`. No released Core wrote such rows, so the
   no-intent case is unreachable, and `Adopt` needs no extra code. If a guard stays, it must use a code from `Adopt`'s
   sync column.
3. Tests: a stop whose link breaks, then `Adopt(id)` with the worker reporting `Running`: the session is `Stopping`, and
   the stop is sent again. With the worker reporting `Exited`: `Exited{HostStop}`. Also a test that no row write records
   `Lost(WorkerUnreachable)` or `Lost(WorkerVersion)`.

### W1 LOW — the one-`Launch` test does not cover a `Launch` during the spawn

`a_worker_accepts_one_launch_in_its_life` sends a second `Launch` after the payload runs, after it exited, and after its
spawn failed. It does not send one while the first spawn is in flight (`PayloadState::Spawning`). That is the R-35 (b)
case, and the one where a second `Launch` would start a second payload. The code is correct today: `on_launch` returns
for every state except `None`. But no test pins `Spawning`, so a change to `matches!(Running | Exited | Failed)` would
pass the test. Add the in-flight case: `Launch`, then a second `Launch` before `Input::Spawned`, and assert that it gives
no action and starts no second spawn.

### Carry

- P7: `botster-guardian-core` still uses `token_proof` in both directions (PR body). The reflection problem of part 2
  applies there when the guardian is adopted.
- The worker side (part 7), `--endpoint` and `--startup-ms`, `RealEdges::connect_worker` and the `Sim` endpoints come in
  later PRs. This reviewer checks there the design's D1 (an equal epoch), D2, D3 and the bounded candidates.

VERDICT: NOT CLEAN (2 open: A1 MEDIUM, W1 LOW)

### Correction after round 1 (same head 6a09f29b) — A1's proposed fix is withdrawn (steward ruling R-36)

Steward ruling R-36 (contracts `main` `c62085f`, read here) supersedes A1's proposed fix. A1's finding stays: `Adopt(id)`
must not be refused with `Unsupported`. R-36 Q3: "an `Adopt(id)` of a `Lost(WorkerUnreachable)` or `Lost(WorkerVersion)`
row is always admitted ... `WrongState` is only for a row in another state".

This reviewer withdraws the fix "keep the `Stopping` intent in `row_state` on the stop path". R-36 Q2: "A `Lost` row keeps
the worker's identity (AD-2, AD-6), not the earlier state", and keeping a `Stopping` intent durably "would change LC-5's
outcome for that path ... it would be an amendment". The P5 package reviewer withdrew the same proposal in F24.

A1's fix, per R-36:
1. `Adopt(id)` is admitted for every `Lost(WorkerUnreachable)` and `Lost(WorkerVersion)` session, with no condition on a
   kept intent.
2. The result follows the worker's report (R-36 Q1): the payload runs → `Running`; the payload ended → `Exited`;
   otherwise → `Lost(reason)` again, with the reason that applies now. Nothing is launched (AD-2: "never restarted in
   place").
3. The kept-intent retry rule of this PR (`row_state`, and the retry that sends a `Launch` or resumes a `Stopping`) does not
   match R-36. `a_retry_after_a_lost_launch_sends_the_launch` and `a_retry_of_a_stopping_row_exits_host_stop_with_no_launch`
   test that rule. Reconcile them with R-36 and with the P5 package reviewer.
4. Tests: a broken-link `Stop`, then `Adopt(id)` with the report `Running` (→ `Running`), `Exited` (→ `Exited`), and
   `NotLaunched` (→ `Lost` again, with no `Launch`). Also the same after a new handle recovers the row.

Design rounds 3 and 4 (`verdicts/stage1/p5-adoption-design.md`) approved the kept-intent retry under R-35. R-36 now
supersedes that part of the design. The design text must follow R-36 in the same PR.

VERDICT (unchanged): NOT CLEAN (2 open: A1 MEDIUM with the R-36 fix, W1 LOW)

## Round 2 — CLEAN on head 36688882

Reviewed head: `36688882d42e24130f4419689f8415d48a7f7bb4`, a fast-forward from `6a09f29b` (7 commits, 13 files,
+611 -136). v1 is still `a0f78fe4`. Steward ruling R-36 (`c62085f`) and its follow-up (`d18b6de`) are read here.

- **A1 CLOSED, per R-36.**
  - `Adopt(id)` is admitted for every `Lost(WorkerUnreachable)` and `Lost(WorkerVersion)` session (`admit.rs:274-288`).
    The `Unsupported` refusal is gone, and `WrongState` stays only for another state.
  - `row_state` and the kept intent are removed. `recorded_state()` is the shown state. `begin_adoption` takes `None` for
    `Adopt(id)`.
  - `adopt_retry` follows the worker's report alone:
    - `Running` → `Running`; `Exited` → `Exited`;
    - `NotLaunched` → `Lost(StartInterrupted)`, follow-up (i);
    - `Spawning` → a wait for the spawn's answer with no `Launch`, bounded by `startup`, follow-up (ii);
    - `LaunchFailed` → the LC-4 outcome, follow-up (iii).
    No `Launch` is sent, and there is no `Stopping` outcome.
  - The row records the posted state (`post_adoption`, and the adopted start's `Running`). A `Lost` row keeps the
    worker's identity, so a later handle may retry it.
  - The tests named in round 1's correction exist (`tests/adoption.rs:877`, `:900`, `:927`, `:946`), including a new
    handle that retries the row of a broken-link stop.
  - `DESIGN.md` parts 5 and 9 follow R-36 and say that it replaces the round 2 retry rule. No kept-intent text is left in
    the code or the design.
- **W1 CLOSED.** `a_worker_accepts_one_launch_in_its_life` now sends a second `Launch` while the first spawn is in
  flight. It asserts no action, and exactly one spawn report for one `Spawned` input.
- **The P5 package reviewer's F23 and F25** are theirs. The delta keeps the link for every non-`Lost` end (F23), and it
  holds an end that comes during `Flow::Adopt` until after `Running` (F25).
- **`.cargo/mutants.toml`** is unchanged. The targeted run at this head (`…020637-57135.log`) passes fmt, clippy
  `-D warnings`, 487 tests, taint, and the slow-profile in-diff mutation run: 157 tested, 139 caught, 0 missed, 0
  TIMEOUT, 18 unviable. That run is not the full pool gate. The full gate on this exact head stays the lead's check.

VERDICT: CLEAN (0 open) at 36688882d42e24130f4419689f8415d48a7f7bb4

## Round 3 — CLEAN on head 4a5a7158

Reviewed head: `4a5a71585d0abb1470c2ffda56b59780272fba0c`. Delta `36688882..4a5a7158`, one commit for the P5 package
reviewer's F26: `adopt.rs` +7 -1, `run.rs` +7, tests and `DESIGN.md`. v1 is still `a0f78fe4`.

- **F26 (theirs) does not change the integration review.** At recovery, a `Lost(WorkerUnreachable)` or
  `Lost(WorkerVersion)` row with an identity and no valid token is `Lost(RegistryCorrupt)`. At the probe, a session with no
  worker identity ends `Lost(StartInterrupted)` with no probe and no connect. The two `token.expect` calls left in
  `adopt.rs` (`:155`, `:182`) are reached only after a probe of an identity. Recovery now refuses an identity without a
  token, and a session that this handle creates always has its token.
- **R-36 admission stays.** A valid `Lost(WorkerUnreachable)` or `Lost(WorkerVersion)` row is still posted as recorded,
  and `Adopt(id)` admits it. Only the rows that cannot be authenticated become `RegistryCorrupt`.
- **Evidence** (`…021336-63942.log`, targeted, not the full pool gate): 489 tests passed; slow-profile in-diff mutants
  160 tested, 142 caught, 0 missed, 0 TIMEOUT, 18 unviable. The full gate on this exact head stays the lead's check.

VERDICT: CLEAN (0 open) at 4a5a71585d0abb1470c2ffda56b59780272fba0c
