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
