# Integration review: #176 (P5 adoption part 2, branch stage1/p5-adopt-2) — DRAFT

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — NOT CLEAN on head dad9a58a

Reviewed head: `dad9a58add3e5103a37db266b9b388a6091aaf37`, range `afdb540f..dad9a58a` (8 commits, 22 files, +1,805 -86).
`afdb540f` merges #174 at `4a5a7158` into #168 at `7550018f`. v1 is still `a0f78fe4`, and neither #174 nor #168 is merged.
Scope: the cross-package parts (`botster-core-link` launch, `botster-worker-core` adoption, the `botster-worker` binary,
`botster-core` real edges, `botster-core-testkit`), and the design carries D1 to D4 and part 7.

### Not mergeable yet, outside the findings

- The base contains #168 at `7550018f`, which is on HOLD (the lead's ruling: no CLEAN until the rebuilt real-PTY
  regression passes the gate). #176 cannot merge before #168 does.
- The PR is a draft. The in-diff run has 13 missed mutants in real-I/O code, which the PR lists as HOLD items for #171.
  No exclusion is added for them, which is correct. They must be caught or excluded with reasons before a CLEAN.

### Checked, no finding

- **D1 (an equal epoch).** `on_candidate_bytes` accepts a hello with `host_epoch >= self.epoch` and the host proof for this
  instance at that epoch (`host_proven`). A refusal closes only the candidate (A11). `adopt` records the epoch, and the
  worker's own proof then uses it.
- **D2 (`--startup-ms`).** `WorkerLaunch` carries `endpoint` and `startup_ms`. `parse` refuses a missing or malformed
  value, with tests. `millis` saturates at `u64::MAX`.
- **D3 (the endpoint's removal).** The host's `remove_endpoint` records a failed unlink in `diagnostics()`
  (`endpoint_unlink_failures`), and a missing endpoint is no failure. The worker binary removes its own endpoint
  (`unlinked`).
- **D4.** `hello_bound` sizes a candidate's decoder from the hello fields, and the fields do not change between
  protocols.
- **Bounded candidates (part 7).** There is one candidate at a time, and a second one is closed at once. A candidate that
  has not passed by `startup` is closed. A candidate's decoder is bounded to one hello. An ending, removing or terminating
  worker takes no candidate. `FrameDecoder::push` stops at the end of a frame, so `&bytes[took..]` passes the bytes after
  the hello to the new link correctly.
- **The fence.** `Action::AdoptLink` drops the old link's unwritten bytes, and `fence` in `io_decisions.rs` drops the
  queued inputs of the old link. `input_fence` retires the old host's requests, and the write counters start again from
  zero.
- **AD-7 self-exit.** `arm_orphan` runs the `startup` deadline while there is no payload and no `Ready` link. It stops when
  a payload or a host comes.
- **The endpoint directory (AD-6).** `open_endpoint_dir` creates `w` with mode `0700`, and it refuses a link, another
  owner, or any group or other bit (it uses `lstat`). The longest endpoint must fit a socket address, or `open` refuses
  the data directory (`InvalidConfig{data_dir}`). Each case has a test.

### C1 MEDIUM — `RealEdges::connect_worker` is a blocking connect in the host's pump

`crates/botster-core/src/real.rs` `connect_worker`:

    let stream =
        retry_interrupted(|| UnixStream::connect(endpoint_path(&self.data_dir, instance)))
            .ok()?;

The doc comment says "a non-blocking connect", and DESIGN.md part 6 says "a non-blocking `mio` connect". But
`std::os::unix::net::UnixStream::connect` is a blocking `connect(2)`. On Linux, a blocking `AF_UNIX` stream connect to a
listener whose backlog is full sleeps until there is room, and the default send timeout has no limit. The host runs every
session in one pump, so one stopped worker (for example `SIGSTOP`, or a debugger) whose backlog has filled stops the
whole host. A loop of `Adopt(id)` retries of `Lost(WorkerUnreachable)`, which AD-2 allows, fills that backlog. BUILD rule
5 requires every wait to be bounded, and the host edges must never block.

Fix: connect with a non-blocking socket (`mio::net::UnixStream::connect`, as the design says). Treat `EAGAIN` (a full
backlog) like a refused connect, as `None`, so the adoption is `Lost(WorkerUnreachable)` and may be retried. Correct the
comment. Add a default-tier test of the decision that maps the connect result (`Ok`, `ECONNREFUSED`, `ENOENT`, `EAGAIN`)
to a link or to `None`.

VERDICT: NOT CLEAN (1 open: C1 MEDIUM; also waits for #168's HOLD and the 13 HOLD mutants)
